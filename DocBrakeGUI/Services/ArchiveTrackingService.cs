using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using DocBrake.NativeInterop;

namespace DocBrake.Services
{
    public class ArchiveTrackingService
    {
        public class ArchiveRecord
        {
            public long Id { get; set; }
            public string ArchivePath { get; set; } = string.Empty;
            public ulong ArchiveSize { get; set; }
            public ulong CreationDate { get; set; }
            public string OriginalLocation { get; set; } = string.Empty;
            public string DestinationLocation { get; set; } = string.Empty;
            public string Description { get; set; } = string.Empty;
            public uint FileCount { get; set; }
        }

        public List<ArchiveRecord> GetAllArchives(string catalogDbPath)
        {
            var archives = new List<ArchiveRecord>();

            int archiveCount;
            IntPtr archivesPtr;

            int result = OpenArcFFI.GetAllArchives(catalogDbPath, out archiveCount, out archivesPtr);

            if (result == 0 && archiveCount > 0 && archivesPtr != IntPtr.Zero)
            {
                try
                {
                    // Calculate the size of each ArchiveRecordInfo struct
                    int structSize = Marshal.SizeOf<OpenArcFFI.ArchiveRecordInfo>();

                    for (int i = 0; i < archiveCount; i++)
                    {
                        IntPtr currentPtr = IntPtr.Add(archivesPtr, i * structSize);
                        OpenArcFFI.ArchiveRecordInfo recordInfo = Marshal.PtrToStructure<OpenArcFFI.ArchiveRecordInfo>(currentPtr);

                        var archiveRecord = new ArchiveRecord
                        {
                            Id = recordInfo.Id,
                            ArchivePath = recordInfo.ArchivePath ?? string.Empty,
                            ArchiveSize = recordInfo.ArchiveSize,
                            CreationDate = recordInfo.CreationDate,
                            OriginalLocation = recordInfo.OriginalLocation ?? string.Empty,
                            DestinationLocation = recordInfo.DestinationLocation ?? string.Empty,
                            Description = recordInfo.Description ?? string.Empty,
                            FileCount = recordInfo.FileCount
                        };

                        archives.Add(archiveRecord);
                    }
                }
                finally
                {
                    // Free the allocated memory
                    OpenArcFFI.FreeArchivesArray(archivesPtr, archiveCount);
                }
            }

            return archives;
        }

        public bool UpdateArchiveDestination(string catalogDbPath, string archivePath, string destinationPath)
        {
            int result = OpenArcFFI.UpdateArchiveDestination(catalogDbPath, archivePath, destinationPath);
            return result == 0;
        }

        public bool ArchiveEntry(string catalogDbPath, long archiveId)
        {
            // TODO: Implement FFI call to mark archive as archived (hidden)
            // This should set an "archived" flag in the database without deleting the record
            // For now, return false to indicate not implemented
            return false;
        }

        public bool DeleteEntry(string catalogDbPath, long archiveId)
        {
            // TODO: Implement FFI call to delete archive entry from database
            // This should permanently remove the record from the catalog database
            // For now, return false to indicate not implemented
            return false;
        }

        public string FormatFileSize(ulong size)
        {
            string[] sizes = { "B", "KB", "MB", "GB", "TB" };
            double len = size;
            int order = 0;
            while (len >= 1024 && order < sizes.Length - 1)
            {
                order++;
                len = len / 1024;
            }

            return $"{len:0.##} {sizes[order]}";
        }

        public DateTime UnixTimeStampToDateTime(ulong unixTimeStamp)
        {
            var dtDateTime = new DateTime(1970, 1, 1, 0, 0, 0, 0, DateTimeKind.Utc);
            dtDateTime = dtDateTime.AddSeconds(unixTimeStamp).ToLocalTime();
            return dtDateTime;
        }
    }
}