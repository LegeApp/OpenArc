import os
import sys
import subprocess
from pathlib import Path
from loky import ProcessPoolExecutor

# --- Configuration ---

# The script will now automatically find 'bpgenc.exe' in its own directory.
# You can override this by uncommenting and setting the path manually.
# BPG_ENCODER_PATH_OVERRIDE = r"C:\path\to\your\bpgenc.exe"

INPUT_FOLDER = r"C:\Users\dk\Desktop\2025-sony\png"
OUTPUT_ROOT = r"C:\Users\dk\Desktop\2025-sony\png\bpg"
BIT_DEPTH = 12

# Determine the number of parallel workers to use.
# os.cpu_count() is a good default to use all available CPU cores.
MAX_WORKERS = os.cpu_count()

# --- Script Logic (No need to edit below this line) ---

def find_encoder():
    """Finds the bpgenc.exe executable."""
    # Check for a manual override path first.
    if 'BPG_ENCODER_PATH_OVERRIDE' in globals():
        encoder_path = Path(BPG_ENCODER_PATH_OVERRIDE)
        if encoder_path.is_file():
            print(f"Using manually specified encoder: {encoder_path}")
            return str(encoder_path)
        else:
            print(f"Error: Manual encoder path not found at '{encoder_path}'")
            sys.exit(1)

    # If no override, look for the exe in the script's directory.
    # We use sys.argv[0] to reliably find the script's path.
    script_path = Path(sys.argv[0]).resolve()
    script_dir = script_path.parent
    encoder_path = script_dir / "bpgenc.exe"

    if not encoder_path.is_file():
        print("---")
        print("Error: 'bpgenc.exe' not found.")
        print(f"Please place 'bpgenc.exe' in the same folder as this script:")
        print(f"-> {script_dir}")
        print("---")
        sys.exit(1) # Exit the script with an error code

    print(f"Found encoder: {encoder_path}")
    return str(encoder_path)

def convert_to_bpg(task_args):
    """
    Worker function to convert an image to BPG.
    Accepts a tuple of arguments to be compatible with the executor.
    Supports PNG and JPEG/JPG input files.
    """
    encoder_path, input_path, output_path, bit_depth = task_args
    
    cmd = [
        encoder_path,
        "-b", str(bit_depth),
        "-o", str(output_path),
        str(input_path)
    ]
    try:
        # Using capture_output=True and text=True to hide subprocess output
        # unless there is an error.
        result = subprocess.run(
            cmd, 
            check=True, 
            capture_output=True, 
            text=True
        )
        print(f"SUCCESS: {input_path} → {output_path}")
    except subprocess.CalledProcessError as e:
        print(f"--- FAILED to convert {input_path} ---")
        print(f"Command: {' '.join(cmd)}")
        print(f"Error: {e}")
        print(f"STDOUT: {e.stdout}")
        print(f"STDERR: {e.stderr}")
        print("-----------------------------------------")


def main():
    """Main function to find files and process them in parallel."""
    bpg_encoder_path = find_encoder()
    
    input_folder = Path(INPUT_FOLDER)
    output_root = Path(OUTPUT_ROOT)

    if not input_folder.is_dir():
        print(f"Error: Input folder not found at '{INPUT_FOLDER}'")
        sys.exit(1)

    # 1. Collect all conversion tasks
    tasks = []
    print("\nScanning for all supported files...")
    for root, _, files in os.walk(input_folder):
        for file in files:
            if file.lower().endswith((".png", ".jpeg", ".jpg")):
                input_path = Path(root) / file
                
                # Recreate the directory structure in the output folder
                relative_path = input_path.parent.relative_to(input_folder)
                output_dir = output_root / relative_path
                output_dir.mkdir(parents=True, exist_ok=True)

                # Define the output file path
                bpg_filename = input_path.with_suffix(".bpg").name
                output_path = output_dir / bpg_filename

                # Add the task arguments as a tuple to our list
                tasks.append((bpg_encoder_path, input_path, output_path, BIT_DEPTH))

    if not tasks:
        print("No supported image files found. Exiting.")
        return

    print(f"Found {len(tasks)} files to convert. Starting parallel processing with {MAX_WORKERS} workers...")

    # 2. Process tasks in parallel using Loky
    # The 'with' statement ensures the pool is properly shut down
    with ProcessPoolExecutor(max_workers=MAX_WORKERS) as executor:
        # map() distributes the tasks from the 'tasks' list to the
        # 'convert_to_bpg' worker function.
        # We wrap it in list() to ensure we wait for all tasks to complete.
        list(executor.map(convert_to_bpg, tasks))

    print("\nAll tasks completed.")

if __name__ == "__main__":
    main()
