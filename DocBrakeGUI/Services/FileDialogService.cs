using Microsoft.Win32;
using System;
using System.Linq;
using System.Runtime.InteropServices;

namespace DocBrake.Services
{
    public class FileDialogService : IFileDialogService
    {
        [DllImport("shell32.dll", SetLastError = true, CharSet = CharSet.Auto)]
        private static extern IntPtr SHBrowseForFolder(ref BROWSEINFO lpbi);

        [DllImport("shell32.dll", CharSet = CharSet.Auto)]
        private static extern bool SHGetPathFromIDList(IntPtr pidl, IntPtr pszPath);

        [DllImport("ole32.dll")]
        private static extern void CoTaskMemFree(IntPtr ptr);

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        private struct BROWSEINFO
        {
            public IntPtr hwndOwner;
            public IntPtr pidlRoot;
            public IntPtr pszDisplayName;
            public string lpszTitle;
            public uint ulFlags;
            public IntPtr lpfn;
            public IntPtr lParam;
            public int iImage;
        }

        private const uint BIF_RETURNONLYFSDIRS = 0x0001;
        public string? OpenFileDialog(string title, string filter, string? initialDirectory = null)
        {
            var dialog = new Microsoft.Win32.OpenFileDialog
            {
                Title = title,
                Filter = filter,
                InitialDirectory = initialDirectory ?? Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments)
            };

            return dialog.ShowDialog() == true ? dialog.FileName : null;
        }

        public string[]? OpenFilesDialog(string title, string filter, string? initialDirectory = null)
        {
            var dialog = new Microsoft.Win32.OpenFileDialog
            {
                Title = title,
                Filter = filter,
                Multiselect = true,
                InitialDirectory = initialDirectory ?? Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments)
            };

            return dialog.ShowDialog() == true ? dialog.FileNames : null;
        }        public string? OpenFolderDialog(string title, string? initialDirectory = null)
        {
            var bi = new BROWSEINFO
            {
                hwndOwner = IntPtr.Zero,
                pidlRoot = IntPtr.Zero,
                pszDisplayName = IntPtr.Zero,
                lpszTitle = title,
                ulFlags = BIF_RETURNONLYFSDIRS,
                lpfn = IntPtr.Zero,
                lParam = IntPtr.Zero,
                iImage = 0
            };

            IntPtr pidl = SHBrowseForFolder(ref bi);
            if (pidl == IntPtr.Zero)
                return null;

            IntPtr path = Marshal.AllocHGlobal(260 * Marshal.SystemDefaultCharSize);
            try
            {
                if (SHGetPathFromIDList(pidl, path))
                {
                    return Marshal.PtrToStringAuto(path);
                }
                return null;
            }
            finally
            {
                CoTaskMemFree(pidl);
                Marshal.FreeHGlobal(path);
            }
        }

        public string? SaveFileDialog(string title, string filter, string? defaultFileName = null, string? initialDirectory = null)
        {
            var dialog = new Microsoft.Win32.SaveFileDialog
            {
                Title = title,
                Filter = filter,
                FileName = defaultFileName ?? string.Empty,
                InitialDirectory = initialDirectory ?? Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments)
            };

            return dialog.ShowDialog() == true ? dialog.FileName : null;
        }
    }
}
