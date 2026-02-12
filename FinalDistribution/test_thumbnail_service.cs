using System;
using System.IO;
using DocBrake.MediaBrowser.Services;

class Program
{
    static void Main(string[] args)
    {
        Console.WriteLine("=== Testing ThumbnailCacheService GPU Initialization ===");

        try
        {
            // Create the service - this should trigger GPU init logging
            var cacheService = new ThumbnailCacheService();
            Console.WriteLine("✓ ThumbnailCacheService created successfully");

            // Try to generate a thumbnail if a test image is provided
            if (args.Length > 0 && File.Exists(args[0]))
            {
                Console.WriteLine($"Testing thumbnail generation for: {args[0]}");
                var result = cacheService.GenerateThumbnail(args[0]);
                Console.WriteLine($"Thumbnail result: {(result ? "SUCCESS" : "FAILED")}");
            }
            else
            {
                Console.WriteLine("No test image provided - GPU init test complete");
            }
        }
        catch (Exception ex)
        {
            Console.WriteLine($"✗ Error: {ex.Message}");
        }

        Console.WriteLine("Press Enter to exit...");
        Console.ReadLine();
    }
}