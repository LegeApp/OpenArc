using System;
using System.Runtime.InteropServices;

namespace DocBrake.Services
{
    public enum GpuBackendType
    {
        None = 0,
        Cuda = 1,
        OpenCL = 2,
        DirectML = 3
    }

    public class NativeGpuService
    {
        // P/Invoke methods to access the native GPU manager
        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasGPU();

        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasCUDA();

        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasOpenCL();

        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern bool NativeHasDirectML();

        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeGetActiveBackend();

        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        [return: MarshalAs(UnmanagedType.LPStr)]
        private static extern string NativeGetActiveBackendName();

        [DllImport("yolo_layout.dll", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        [return: MarshalAs(UnmanagedType.LPStr)]
        private static extern string NativeGetDeviceName();

        private static readonly Lazy<NativeGpuService> _instance = new Lazy<NativeGpuService>(() => new NativeGpuService());
        public static NativeGpuService Instance => _instance.Value;

        private bool _initialized = false;
        private bool _nativeLibraryLoaded = false;

        private NativeGpuService()
        {
            try
            {
                // Try to load the native library explicitly
                _nativeLibraryLoaded = LoadNativeLibrary();
                _initialized = _nativeLibraryLoaded;
                
                if (_initialized)
                {
                    // Test a simple function to verify the library is working
                    _initialized = SafeNativeCall(NativeHasGPU, false);
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Error initializing: {ex}");
                _initialized = false;
                _nativeLibraryLoaded = false;
            }
        }

        private bool LoadNativeLibrary()
        {
            try
            {
                var assemblyLocation = System.Reflection.Assembly.GetExecutingAssembly().Location;
                var assemblyDirectory = System.IO.Path.GetDirectoryName(assemblyLocation) ?? string.Empty;
                var dllPath = System.IO.Path.Combine(assemblyDirectory, "yolo_layout.dll");
                
                if (!System.IO.File.Exists(dllPath))
                {
                    System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Native library not found at: {dllPath}");
                    return false;
                }
                
                // Try to load the library
                if (NativeLibrary.TryLoad(dllPath, out _))
                {
                    System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Successfully loaded native library: {dllPath}");
                    return true;
                }
                
                System.Diagnostics.Trace.WriteLine($"[NativeGpuService] Failed to load native library: {dllPath}");
                return false;
            }
            catch (Exception ex)
            {
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

        public string ActiveBackendName => SafeNativeCall(NativeGetActiveBackendName, "CPU");
        public string DeviceName => SafeNativeCall(NativeGetDeviceName, "CPU");

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
