using System;

namespace DocBrake.Models
{
    public class ArchiveEntryExtractRequest
    {
        public string ArchivePath { get; set; } = string.Empty;
        public string EntryName { get; set; } = string.Empty;
        public string OutputPath { get; set; } = string.Empty;
    }
}
