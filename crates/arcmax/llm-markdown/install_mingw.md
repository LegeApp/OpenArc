# Installing MinGW64 Toolchain

You need to install the MinGW64 toolchain using MSYS2. Follow these steps:

## Method 1: Using MSYS2 Terminal (Recommended)

1. Open MSYS2 MINGW64 terminal (you can find it in Start Menu or run `C:\msys64\mingw64.exe`)

2. Run the following command to install the MinGW64 toolchain:
```bash
pacman -S mingw-w64-x86_64-gcc
```

3. This will install gcc, g++, dlltool, ar, and other necessary tools.

## Method 2: Using PowerShell (if you prefer)

1. Open PowerShell as Administrator

2. Navigate to MSYS2 usr/bin directory:
```powershell
cd C:\msys64\usr\bin
```

3. Run the pacman command:
```powershell
.\pacman.exe -S mingw-w64-x86_64-gcc
```

## After Installation

Once installed, add `C:\msys64\mingw64\bin` to your Windows PATH environment variable, or we can configure Cargo to use it directly.

Then try building again with:
```powershell
cargo build
```
