using System;
using System.Runtime.InteropServices;

namespace DocBrake.Services
{
    public static class DebugLogger
    {
        public static void Log(string message)
        {
            try
            {
                var logPath = System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "debug.log");
                var timestamp = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff");
                System.IO.File.AppendAllText(logPath, $"[{timestamp}] {message}\n");
            }
            catch
            {
                // Ignore logging errors
            }
        }
    }

    public enum GpuBackendType
    {
        None = 0,
        Cuda = 1,
        OpenCL = 2,
        DirectML = 3
    }

    public class NativeGpuService
    {
        // P/Invoke methods to access the native GPU manager from bpg_viewer.dll
        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasGPU();

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasCUDA();

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasOpenCL();

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasDirectML();

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeGetActiveBackend();

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeGetActiveBackendName();

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeGetDeviceName();

        private static readonly Lazy<NativeGpuService> _instance = new Lazy<NativeGpuService>(() => new NativeGpuService());
        public static NativeGpuService Instance => _instance.Value;

        private bool _initialized = false;
        private bool _nativeLibraryLoaded = false;

        private NativeGpuService()
        {
            DebugLogger.Log("NativeGpuService constructor starting");
            
            try
            {
                // Try to load the library explicitly
                DebugLogger.Log("Attempting to load bpg_viewer.dll");
                _nativeLibraryLoaded = LoadNativeLibrary();
                _initialized = _nativeLibraryLoaded;
                
                DebugLogger.Log($"Library load result: {_nativeLibraryLoaded}");
                
                if (_initialized)
                {
                    // Test a simple function to verify the library is working
                    DebugLogger.Log("Testing NativeHasGPU function");
                    _initialized = SafeNativeCall(NativeHasGPU, false);
                    DebugLogger.Log($"NativeHasGPU test result: {_initialized}");
                    
                    // Log initialization result
                    var initMsg = $"GPU Library Load: {(_initialized ? "SUCCESS" : "FAILED")}";
                    System.Diagnostics.Debug.WriteLine(initMsg);
                    System.Diagnostics.Trace.WriteLine(initMsg);
                    Console.WriteLine(initMsg);
                    Console.Out.Flush();
                    DebugLogger.Log(initMsg);
                }
                else
                {
                    var failMsg = "GPU Library Load: FAILED - Library not found or couldn't load";
                    System.Diagnostics.Debug.WriteLine(failMsg);
                    System.Diagnostics.Trace.WriteLine(failMsg);
                    Console.WriteLine(failMsg);
                    Console.Out.Flush();
                    DebugLogger.Log(failMsg);
                }
            }
            catch (Exception ex)
            {
                var errorMsg = $"GPU Library Load: EXCEPTION - {ex.Message}";
                System.Diagnostics.Trace.WriteLine(errorMsg);
                System.Diagnostics.Debug.WriteLine(errorMsg);
                Console.WriteLine(errorMsg);
                Console.Out.Flush();
                DebugLogger.Log(errorMsg);
                DebugLogger.Log($"Exception stack trace: {ex.StackTrace}");
                _initialized = false;
                _nativeLibraryLoaded = false;
            }
            
            DebugLogger.Log($"NativeGpuService constructor completed. Initialized: {_initialized}");
        }

        private bool LoadNativeLibrary()
        {
            DebugLogger.Log("LoadNativeLibrary starting");
            
            try
            {
                var assemblyLocation = System.Reflection.Assembly.GetExecutingAssembly().Location;
                var assemblyDirectory = System.IO.Path.GetDirectoryName(assemblyLocation) ?? string.Empty;
                var dllPath = System.IO.Path.Combine(assemblyDirectory, "bpg_viewer.dll");
                
                DebugLogger.Log($"Looking for DLL at: {dllPath}");
                
                if (!System.IO.File.Exists(dllPath))
                {
                    DebugLogger.Log("DLL file not found");
                    System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Native library not found at: {dllPath}");
                    return false;
                }
                
                DebugLogger.Log("DLL file found, attempting to load");
                
                // Try to load the library
                if (NativeLibrary.TryLoad(dllPath, out _))
                {
                    DebugLogger.Log("DLL loaded successfully via NativeLibrary.TryLoad");
                    System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Successfully loaded native library: {dllPath}");
                    return true;
                }
                
                DebugLogger.Log("NativeLibrary.TryLoad failed");
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Failed to load native library: {dllPath}");
                return false;
            }
            catch (Exception ex)
            {
                DebugLogger.Log($"LoadNativeLibrary exception: {ex.Message}");
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Error loading native library: {ex}");
                return false;
            }
        }

        public bool Initialize()
        {
            try
            {
                // Call native initialization if needed
                _initialized = true;
                return true;
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error initializing GPU service: {ex.Message}");
                _initialized = false;
                return false;
            }
        }

        public bool HasGpu => SafeNativeCall(NativeHasGPU, false);
        public bool HasCuda => SafeNativeCall(NativeHasCUDA, false);
        public bool HasOpenCL => SafeNativeCall(NativeHasOpenCL, false);
        public bool HasDirectML => SafeNativeCall(NativeHasDirectML, false);
        
        public GpuBackendType ActiveBackend => SafeNativeCall(() => 
            (GpuBackendType)NativeGetActiveBackend(), GpuBackendType.None);

        public string ActiveBackendName => SafeNativeCall(() => {
            var ptr = NativeGetActiveBackendName();
            return ptr != IntPtr.Zero ? Marshal.PtrToStringAnsi(ptr) ?? "CPU" : "CPU";
        }, "CPU");
        
        public string DeviceName => SafeNativeCall(() => {
            var ptr = NativeGetDeviceName();
            return ptr != IntPtr.Zero ? Marshal.PtrToStringAnsi(ptr) ?? "CPU" : "CPU";
        }, "CPU");

        private T SafeNativeCall<T>(Func<T> nativeCall, T defaultValue, [System.Runtime.CompilerServices.CallerMemberName] string methodName = "")
        {
            if (!_initialized || !_nativeLibraryLoaded)
            {
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] {methodName} - Native library not initialized or loaded");
                return defaultValue;
            }

            try
            {
                var result = nativeCall();
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] {methodName} - Success: {result}");
                return result;
            }
            catch (DllNotFoundException dllEx)
            {
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] {methodName} - DllNotFoundException: {dllEx.Message}");
                return defaultValue;
            }
            catch (EntryPointNotFoundException entryEx)
            {
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] {methodName} - EntryPointNotFoundException: {entryEx.Message}");
                return defaultValue;
            }
            catch (Exception ex)
            {
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] {methodName} - Error: {ex}");
                return defaultValue;
            }
        }
    }
}
