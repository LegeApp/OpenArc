using System;
using System.Runtime.InteropServices;

// Get MTP devices via Shell.Application COM
var shellType = Type.GetTypeFromProgID("Shell.Application");
if (shellType == null) { Console.WriteLine("Shell.Application not found"); return; }

dynamic shell = Activator.CreateInstance(shellType);
dynamic myComputer = shell.Namespace(17);

Console.WriteLine("Items in My Computer:");
foreach (dynamic item in myComputer.Items())
{
    string name = item.Name;
    string path = item.Path;
    Console.WriteLine($"  Name: {name}");
    Console.WriteLine($"  Path: {path}");
    
    if (path.StartsWith("::") && (path.ToLower().Contains("usb") || path.ToLower().Contains("vid_")))
    {
        Console.WriteLine("  [DETECTED AS MTP DEVICE]");
        
        // Try to enumerate children (storages)
        try {
            dynamic deviceFolder = item.GetFolder;
            if (deviceFolder != null) {
                Console.WriteLine("  Storages:");
                foreach (dynamic storage in deviceFolder.Items()) {
                    Console.WriteLine($"    - {storage.Name}: {storage.Path}");
                }
            }
        } catch {}
    }
    Console.WriteLine();
}
Marshal.ReleaseComObject(shell);
