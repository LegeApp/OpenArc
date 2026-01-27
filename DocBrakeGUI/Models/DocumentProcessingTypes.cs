namespace DocBrake.NativeInterop
{
    /// <summary>
    /// Progress information for archive processing.
    /// Used by both OpenArc FFI and C# UI progress reporting.
    /// </summary>
    public class DocumentProcessingProgress
    {
        public int Current { get; set; } = 0;
        public int Total { get; set; } = 0;
        public string Status { get; set; } = string.Empty;
        public double Percentage => Total > 0 ? (double)Current / Total * 100.0 : 0.0;
    }

    /// <summary>
    /// Result of an archive processing operation.
    /// </summary>
    public class DocumentProcessingResult
    {
        public bool Success { get; set; } = false;
        public string OutputPath { get; set; } = string.Empty;
        public string Error { get; set; } = string.Empty;
    }
}
