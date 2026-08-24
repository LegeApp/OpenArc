{-# LINE 1 "_TABI/TABI.hsc" #-}
{-# OPTIONS -fglasgow-exts -cpp #-}
module TABI (
  Value(..),
  FUNCTION,
  C_FUNCTION,
  ELEMENT(..),
  WideString(..),
--  dump,
--  dump_n,
  callret,               -- :: Value a => C_FUNCTION -> [ELEMENT] -> IO a
  call,                  -- :: C_FUNCTION -> [ELEMENT] -> IO Int
  required,
  optional,
  parameter,
) where

import Prelude hiding (catch)
import Control.Exception
import Control.Monad
import Data.IORef
import Data.Int
import Data.Maybe
import Data.Word
import Foreign
import Foreign.C

-- Include definitions of types and constants from C++ code



-- Rules of serialization for values that may be passed via TABI
class Value a where
  typeOf :: a -> Int32                          -- ^Integer constant that represents this type of values
  pokeValue :: Ptr () -> a -> IO (IO ())        -- ^Write value to given memory address and return action that will free memory buffers allocated for this value
  peekValue :: Int32 -> Ptr () -> IO (Maybe a)  -- ^Read value from given memory address if its type allows conversion to type `a`


instance Value Bool where
  typeOf _ = 6
{-# LINE 40 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int32) (fromIntegral$ fromEnum a) >> return doNothing
  peekValue t ptr | t == 6  =  peek (castPtr ptr :: Ptr Int32) >>= return.Just .toEnum.fromIntegral
{-# LINE 42 "_TABI/TABI.hsc" #-}
                  | otherwise                =  return Nothing

instance Value Int where
  typeOf _ = 1
{-# LINE 46 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 48 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value Integer where
  typeOf _ = 1
{-# LINE 52 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 54 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value CUInt where
  typeOf _ = 1
{-# LINE 58 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 60 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value Int32 where
  typeOf _ = 1
{-# LINE 64 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 66 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value Word32 where
  typeOf _ = 1
{-# LINE 70 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 72 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value Int64 where
  typeOf _ = 1
{-# LINE 76 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 78 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value Word64 where
  typeOf _ = 1
{-# LINE 82 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr Int64) (fromIntegral a) >> return doNothing
  peekValue t ptr | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 84 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing


instance Value Double where
  typeOf _ = 2
{-# LINE 89 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr :: Ptr CDouble) (fromRational$ toRational a) >> return doNothing
  peekValue t ptr | t == 2 =  peek (castPtr ptr :: Ptr CDouble) >>= return.Just .fromRational.toRational
{-# LINE 91 "_TABI/TABI.hsc" #-}
                  | t == 1  =  peek (castPtr ptr :: Ptr Int64) >>= return.Just .fromIntegral
{-# LINE 92 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value String where
  typeOf _ = 3
{-# LINE 96 "_TABI/TABI.hsc" #-}
  pokeValue ptr str  =  do cstr <- raiseIfNull "failed malloc for TABI_STRING" (newCAString str)
                           poke (castPtr ptr) cstr
                           return (free cstr)
  peekValue t ptr | t == 3   =  peek (castPtr ptr :: Ptr CString) >>= peekCAString >>= return.Just
{-# LINE 100 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

newtype WideString a = W a

instance Value (WideString String) where
  typeOf _ = 7
{-# LINE 106 "_TABI/TABI.hsc" #-}
  pokeValue ptr (W str)  =  do cstr <- raiseIfNull "failed malloc for TABI_WIDE_STRING" (newCWString str)
                               poke (castPtr ptr) cstr
                               return (free cstr)
  peekValue t ptr | t == 7   =  peek (castPtr ptr :: Ptr CWString) >>= peekCWString >>= return.Just. W
{-# LINE 110 "_TABI/TABI.hsc" #-}
                  | otherwise                        =  return Nothing

instance Value (Ptr a) where
  typeOf _ = 4
{-# LINE 114 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr) a >> return doNothing
  peekValue t ptr | t == 4      =  peek (castPtr ptr) >>= return.Just
{-# LINE 116 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing

instance Value (FunPtr a) where
  typeOf _ = 5
{-# LINE 120 "_TABI/TABI.hsc" #-}
  pokeValue ptr a  =  poke (castPtr ptr) a >> return doNothing
  peekValue t ptr | t == 5  =  peek (castPtr ptr) >>= return.Just
{-# LINE 122 "_TABI/TABI.hsc" #-}
                  | otherwise                   =  return Nothing




-- |TABI function type
type FUNCTION    =  Ptr ELEMENT -> IO Int
type C_FUNCTION  =  Ptr ELEMENT -> IO CInt
foreign import ccall safe "wrapper"
   mkFUNCTION_WRAPPER :: C_FUNCTION -> IO (FunPtr C_FUNCTION)
foreign import ccall "dynamic"
   mkFUNCTION_DYNAMIC :: FunPtr C_FUNCTION -> C_FUNCTION

instance Value C_FUNCTION where
  typeOf _ = 5
{-# LINE 137 "_TABI/TABI.hsc" #-}
  -- Write pointer to C_FUNCTION to memory
  pokeValue ptr c_callback =  do funptr_c_callback <- mkFUNCTION_WRAPPER c_callback
                                 poke (castPtr ptr) funptr_c_callback
                                 return (freeHaskellFunPtr funptr_c_callback)
  -- Read pointer to C_FUNCTION
  peekValue t ptr | t == 5  =
{-# LINE 143 "_TABI/TABI.hsc" #-}
                                                   do c_callback <- mkFUNCTION_DYNAMIC `fmap` peek (castPtr ptr)
                                                      return (Just c_callback)
                  | otherwise                   =  return Nothing


instance Value FUNCTION where
  typeOf _ = 5
{-# LINE 150 "_TABI/TABI.hsc" #-}
  -- Write to memory C wrapper around Haskell FUNCTION
  pokeValue ptr callback =  do let c_callback params  =  fromIntegral `fmap` callback params
                               pokeValue ptr (c_callback :: C_FUNCTION)
  -- Read pointer to C_FUNCTION and convert it to Haskell FUNCTION
  peekValue t ptr = do c_callback  <- peekValue t ptr
                       return$ fmap (fmap fromIntegral.) (c_callback :: Maybe C_FUNCTION)



-- |Basic TABI value, with name and type information
data ELEMENT = forall a. (Value a) => Pair String a

-- |Dump contents of ELEMENTs array
--dump ptr = dump_n ptr 0
--foreign import ccall safe "tabi_dump"
--  dump_n :: Ptr ELEMENT -> Int -> IO ()

-- Convert pointer to TABI_ELEMENT to pointer to one of its fields
nameField  = (\hsc_ptr -> hsc_ptr `plusPtr` 0)
{-# LINE 169 "_TABI/TABI.hsc" #-}
typeField  = (\hsc_ptr -> hsc_ptr `plusPtr` 8)
{-# LINE 170 "_TABI/TABI.hsc" #-}
valueField = (\hsc_ptr -> hsc_ptr `plusPtr` 16)
{-# LINE 171 "_TABI/TABI.hsc" #-}

-- |Write ELEMENT to array[i] and return freeing action
pokeELEMENT array i (Pair n v) = do
  let ptr = array `plusPtr` (i * (32))    -- address of array[i]
{-# LINE 175 "_TABI/TABI.hsc" #-}
  action1 <- pokeValue (nameField  ptr) n
  (flip onException) action1 $ do                         -- on exception free memory immediately
    ;          poke      (typeField  ptr) (typeOf v)
    action2 <- pokeValue (valueField ptr) v
    return (action1 >> action2)




-- |Call that returns value of arbitrary type
callret :: Value a => C_FUNCTION -> [ELEMENT] -> IO a
callret server params = do
  result <- newIORef$ error "TABI.callret: undefined result"          -- create variable to store result of call
  let return_callback p = do                                          -- callback used to return result of call
        writeIORef result =<< TABI.required p "result"
        return 0
  call server (Pair "return" (return_callback::FUNCTION) : params)    -- add return callback to params list
  readIORef result

-- |Call server passing params using TABI convention
call :: C_FUNCTION -> [ELEMENT] -> IO Int
call server params = do
  let size x = (x+length params) * (32)        -- memory required for serialization of all params plus x more values
{-# LINE 198 "_TABI/TABI.hsc" #-}
  allocaBytes (size 1) $ \c_params -> do                       -- alloc C-style array to store all params
    actions <- zipWithM (pokeELEMENT c_params) [0..] params      -- write params to the array
    (flip finally) (sequence_ actions) $ do                      -- free at the end all memory used for marshalling params
      poke (nameField (c_params `plusPtr` size 0)) nullPtr         -- put NULL marker at the N+1 array position
      server c_params >>= return . fromIntegral                    -- call server





-- |Unmarshall required parameter
required params name        =  parameter params name (raise ("required parameter <<"++name++">> not found"))

-- |Unmarshall optional parameter with default value deflt
optional params name deflt  =  parameter params name (return deflt)

-- |Unmarshall parameter from table executing default_action when it's not found
parameter params name default_action = do
  ptr <- find params name
  if ptr==nullPtr then default_action else do
    t <- peek        (typeField ptr)
    v <- peekValue t (valueField ptr)
    case v of
      Just value -> return value
      Nothing    -> raise ("parameter <<"++name++">>: type mismatch (expected "++show (typeOf (fromJust v))++", actual "++show t++")")

-- |Search C array of TABI_ELEMENTs for element having given name
find c_params name = go c_params
  where go ptr = do cstr <- peek (nameField ptr)
                    if cstr==nullPtr
                      then return nullPtr
                      else do n <- peekCAString cstr
                              if n==name
                                then return ptr
                                else go (ptr `plusPtr` (32))
{-# LINE 233 "_TABI/TABI.hsc" #-}





-- |Simple exception raising
raise = ioError . userError . ("TABI: "++)

-- |NULL pointer checking
raiseIfNull str = throwIfNull ("TABI: "++str)

-- |Do nothing :)
doNothing = return ()

-- |Transform exception raised by computation
mapExceptionM f io = io `catch` \e -> throw (f e)
