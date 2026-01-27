import os
import sys
import subprocess
import shutil
import time
import argparse
from pathlib import Path
from loky import ProcessPoolExecutor
from colorama import init, Fore, Style
from dataclasses import dataclass
from typing import List, Tuple

init(autoreset=True)

# --- Configuration ---
INPUT_FOLDER = r"D:\s24-temp2"
OUTPUT_ROOT = r"D:\s24-temp2\bpg"
BIT_DEPTH = 10
MAX_WORKERS = os.cpu_count()

class Colors:
    HEADER    = Fore.CYAN + Style.BRIGHT
    SUCCESS   = Fore.WHITE
    BEST      = Fore.GREEN + Style.BRIGHT
    WORST     = Fore.MAGENTA + Style.BRIGHT
    INFO      = Fore.LIGHTCYAN_EX
    STATS     = Fore.LIGHTYELLOW_EX
    AVG       = Fore.LIGHTBLUE_EX
    TOTAL     = Fore.LIGHTGREEN_EX
    RESET     = Style.RESET_ALL

@dataclass
class ConversionResult:
    input_path: Path
    output_path: Path
    input_size: int
    output_size: int
    time_taken: float
    ratio: float
    saved_bytes: int

def find_encoder() -> str:
    encoders = {
        'win32': 'bpgenc.exe',
        'linux': 'bpgenc',
        'darwin': 'bpgenc'
    }

    encoder_name = None
    for platform, name in encoders.items():
        if sys.platform.startswith(platform):
            encoder_name = name
            break

    if not encoder_name:
        print(f"{Fore.RED}Unsupported platform: {sys.platform}{Colors.RESET}")
        sys.exit(1)

    # Try PATH
    if shutil.which(encoder_name):
        print(f"{Colors.INFO}Using {encoder_name} from PATH{Colors.RESET}")
        return encoder_name

    # Try script directory
    script_dir = Path(sys.argv[0]).resolve().parent
    encoder_path = script_dir / encoder_name
    if encoder_path.is_file():
        print(f"{Colors.INFO}Using {encoder_name} from script directory{Colors.RESET}")
        return str(encoder_path)

    print(f"{Fore.RED}Error: '{encoder_name}' not found!{Colors.RESET}")
    sys.exit(1)

def convert_to_bpg(task_args) -> ConversionResult:
    encoder_path, input_path, output_path, bit_depth, codec = task_args
    start_time = time.perf_counter()

    input_size = input_path.stat().st_size

    try:
        # Build encoder command
        cmd = [
            encoder_path,
            "-b", str(bit_depth),
            "-o", str(output_path),
            "-c", "ycbcr",
            "-f", "444",
            "-m", "9"
        ]

        if codec == "jctvc":
            cmd.extend(["-e", "jctvc"])
        else:
            cmd.extend(["-e", "x265"])

        cmd.append(str(input_path))  # direct file input

        subprocess.run(cmd, capture_output=True, check=True)

        output_size = output_path.stat().st_size
        time_taken = time.perf_counter() - start_time
        ratio = output_size / input_size
        saved_bytes = input_size - output_size

        print(
            f"{Colors.SUCCESS}Converted {input_path.name} → {output_path.name} "
            f"({input_size/1024:.1f}→{output_size/1024:.1f} KB, {ratio:.1%}) "
            f"in {time_taken:.2f}s{Colors.RESET}"
        )

        return ConversionResult(
            input_path=input_path,
            output_path=output_path,
            input_size=input_size,
            output_size=output_size,
            time_taken=time_taken,
            ratio=ratio,
            saved_bytes=saved_bytes
        )

    except Exception as e:
        time_taken = time.perf_counter() - start_time
        print(f"{Fore.RED}Failed: {input_path.name} ({time_taken:.2f}s)\n   {e}{Colors.RESET}")

        return ConversionResult(
            input_path=input_path,
            output_path=output_path,
            input_size=input_size,
            output_size=0,
            time_taken=time_taken,
            ratio=999.0,
            saved_bytes=-input_size
        )

def format_bytes(b: int) -> str:
    for unit in ['B', 'KB', 'MB', 'GB']:
        if abs(b) < 1024:
            return f"{b:,.2f} {unit}"
        b /= 1024
    return f"{b:,.2f} TB"

def parse_args():
    parser = argparse.ArgumentParser(description='BPG Batch Encoder')
    parser.add_argument(
        '--codec',
        type=str,
        choices=['x265', 'jctvc'],
        default='x265',
        help='x265 (default, faster) or jctvc (slower but higher quality)'
    )
    return parser.parse_args()

def main():
    args = parse_args()
    print(f"{Colors.HEADER}BPG Batch Encoder • {args.codec.upper()} • 10-bit • PNG + JPG Only{Colors.RESET}\n")

    encoder = find_encoder()

    input_folder = Path(INPUT_FOLDER)
    output_root = Path(OUTPUT_ROOT)
    output_root.mkdir(parents=True, exist_ok=True)

    if not input_folder.exists():
        print(f"{Fore.RED}Input folder not found: {INPUT_FOLDER}{Colors.RESET}")
        sys.exit(1)

    print(f"{Colors.INFO}Scanning PNG/JPG recursively...{Colors.RESET}")

    tasks: List[Tuple[str, Path, Path, int]] = []
    valid_exts = {".png", ".jpg", ".jpeg"}

    # Faster directory walk
    for root, dirs, files in os.walk(input_folder):
        for f in files:
            file_path = Path(root) / f
            if file_path.suffix.lower() in valid_exts:
                rel = file_path.relative_to(input_folder)
                out_path = output_root / rel.parent / (file_path.stem + ".bpg")
                out_path.parent.mkdir(parents=True, exist_ok=True)
                tasks.append((encoder, file_path, out_path, BIT_DEPTH, args.codec))

    if not tasks:
        print(f"{Fore.YELLOW}No PNG/JPG files found.{Colors.RESET}")
        return

    print(f"{Colors.INFO}Queued {len(tasks)} files → Starting with {MAX_WORKERS} workers...\n{Colors.RESET}")

    total_start = time.perf_counter()

    with ProcessPoolExecutor(max_workers=MAX_WORKERS) as executor:
        results = list(executor.map(convert_to_bpg, tasks))

    total_time = time.perf_counter() - total_start

    successful = [r for r in results if r.output_size > 0 and r.ratio < 10]
    failed = len(results) - len(successful)

    if not successful:
        print(f"{Fore.RED}No files converted successfully.{Colors.RESET}")
        return

    ratios = [r.ratio for r in successful]
    total_input = sum(r.input_size for r in successful)
    total_output = sum(r.output_size for r in successful)
    total_saved = total_input - total_output
    avg_ratio = sum(ratios) / len(ratios)

    best = min(successful, key=lambda x: x.ratio)
    worst = max(successful, key=lambda x: x.ratio)
    most_saved = max(successful, key=lambda x: x.saved_bytes)

    print(f"\n{Colors.HEADER}Conversion Complete!{Colors.RESET}\n")
    print(f"{Colors.TOTAL}Processed   : {len(results)} files ({len(successful)} success, {failed} failed)")
    print(f"{Colors.TOTAL}Total time  : {total_time:.2f}s "
          f"({len(successful)/total_time:.1f} files/sec)\n")

    print(f"{Colors.INFO}Size Summary:{Colors.RESET}")
    print(f"   Original → {format_bytes(total_input)}")
    print(f"   BPG      → {format_bytes(total_output)}")
    print(f"   Saved    → {Colors.BEST}{format_bytes(total_saved)} "
          f"({total_saved/total_input:.1%} smaller){Colors.RESET}\n")

    print(f"{Colors.AVG}Avg ratio   : {avg_ratio:.1%}{Colors.RESET}")
    print(f"{Colors.BEST}Best        : {best.ratio:.1%} ← {best.input_path.name}{Colors.RESET}")
    print(f"{Colors.WORST}Worst       : {worst.ratio:.1%} ← {worst.input_path.name}{Colors.RESET}")
    print(f"{Colors.BEST}Most saved  : {format_bytes(most_saved.saved_bytes)} ← {most_saved.input_path.name}{Colors.RESET}")

    print(f"\n{Colors.HEADER}Output → {output_root}{Colors.RESET}")

# Auto-install missing deps
if __name__ == "__main__":
    missing = []
    for pkg in ["colorama", "loky"]:
        try:
            __import__(pkg)
        except ImportError:
            missing.append(pkg)

    if missing:
        print(f"{Fore.YELLOW}Installing: {', '.join(missing)}")
        subprocess.check_call([sys.executable, "-m", "pip", "install", *missing])
        print(f"{Fore.GREEN}Dependencies installed. Restart the script.")
        sys.exit(0)

    main()
