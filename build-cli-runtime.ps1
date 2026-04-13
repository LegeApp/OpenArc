param(
    [string]$Target = "x86_64-pc-windows-gnu",
    [string]$OutDir = "$PSScriptRoot\dist\cli-runtime",
    [switch]$SkipBuild,
    [switch]$SkipFfmpegDllBuild
)

$ErrorActionPreference = "Stop"

function Resolve-Tool {
    param([string[]]$Candidates)
    foreach ($c in $Candidates) {
        if ([string]::IsNullOrWhiteSpace($c)) { continue }
        if (Test-Path $c) { return $c }
        $cmd = Get-Command $c -ErrorAction SilentlyContinue
        if ($cmd) { return $cmd.Source }
    }
    return $null
}

function Get-DllImports {
    param(
        [string]$ObjdumpExe,
        [string]$BinaryPath
    )

    if (-not (Test-Path $BinaryPath)) { return @() }
    $lines = & $ObjdumpExe -p $BinaryPath 2>$null
    if (-not $lines) { return @() }

    $imports = @()
    foreach ($line in $lines) {
        if ($line -match "DLL Name:\s+(.+)$") {
            $imports += $Matches[1].Trim()
        }
    }
    return $imports
}

Write-Host "=== OpenArc CLI Runtime Build ===" -ForegroundColor Cyan
Write-Host "Target: $Target" -ForegroundColor Gray
Write-Host "Output: $OutDir" -ForegroundColor Gray
Write-Host ""

$cargo = Resolve-Tool @(
    "C:\Users\dk\.rustup\toolchains\stable-x86_64-pc-windows-gnu\bin\cargo.exe",
    "cargo.exe",
    "cargo"
)
if (-not $cargo) {
    throw "cargo not found. Install Rust or add cargo to PATH."
}

$gcc = Resolve-Tool @(
    "C:\msys64\mingw64\bin\gcc.exe",
    "gcc.exe",
    "gcc"
)
if (-not $gcc) {
    throw "gcc not found. Install MSYS2 mingw64 toolchain."
}

$objdump = Resolve-Tool @(
    "C:\msys64\mingw64\bin\objdump.exe",
    "objdump.exe",
    "objdump"
)
if (-not $objdump) {
    throw "objdump not found. Install MSYS2 mingw64 binutils."
}

if (-not $SkipBuild) {
    Write-Host "Building openarc CLI (release)..." -ForegroundColor Green
    $env:OPENARC_BUILD_GUI = "0"
    & $cargo build --release -p openarc --bin openarc --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed"
    }
}

$exePath = Join-Path $PSScriptRoot "target\$Target\release\openarc.exe"
if (-not (Test-Path $exePath)) {
    throw "openarc.exe not found at $exePath"
}

$bpgDllPath = Join-Path $PSScriptRoot "native\BPG\libbpg-0.9.8\openarc_bpg.dll"
if (-not (Test-Path $bpgDllPath)) {
    throw "openarc_bpg.dll not found at $bpgDllPath. Build it first via native\BPG\libbpg-0.9.8\build_openarc_combined_dll.ps1."
}

$ffmpegDllPath = Join-Path $PSScriptRoot "dist\openarc_ffmpeg.dll"
if (-not $SkipFfmpegDllBuild) {
    Write-Host "Building openarc_ffmpeg.dll..." -ForegroundColor Green
    Push-Location $PSScriptRoot
    try {
        & $gcc -shared -O2 -o "dist\openarc_ffmpeg.dll" "crates\codecs\ffmpeg_wrapper.c" `
            "-IC:\msys64\mingw64\include" "-LC:\msys64\mingw64\lib" `
            -lavformat -lavcodec -lswscale -lswresample -lavutil `
            -lole32 -luser32 -lbz2 -lz -lsecur32 -lncrypt -lcrypt32 -lws2_32 `
            -liconv -latomic -lx264 -lx265
        if ($LASTEXITCODE -ne 0) {
            throw "gcc failed to build openarc_ffmpeg.dll"
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $ffmpegDllPath)) {
    throw "openarc_ffmpeg.dll not found at $ffmpegDllPath"
}

if (-not (Test-Path $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
}

Write-Host "Staging primary binaries..." -ForegroundColor Green
Copy-Item $exePath (Join-Path $OutDir "openarc.exe") -Force
Copy-Item $bpgDllPath (Join-Path $OutDir "openarc_bpg.dll") -Force
Copy-Item $ffmpegDllPath (Join-Path $OutDir "openarc_ffmpeg.dll") -Force

$searchDirs = @(
    $OutDir,
    (Join-Path $PSScriptRoot "dist"),
    (Join-Path $PSScriptRoot "target\$Target\release"),
    (Join-Path $PSScriptRoot "native\BPG\libbpg-0.9.8"),
    "C:\msys64\mingw64\bin"
) | Where-Object { Test-Path $_ }

$systemDlls = @(
    "KERNEL32.DLL",
    "MSVCRT.DLL",
    "ADVAPI32.DLL",
    "CRYPT32.DLL",
    "BCRYPT.DLL",
    "BCRYPTPRIMITIVES.DLL",
    "DNSAPI.DLL",
    "DWRITE.DLL",
    "GDI32.DLL",
    "GDIPLUS.DLL",
    "IPHLPAPI.DLL",
    "MSIMG32.DLL",
    "NCRYPT.DLL",
    "NETAPI32.DLL",
    "NTDLL.DLL",
    "OLE32.DLL",
    "OLEAUT32.DLL",
    "PDH.DLL",
    "POWRPROF.DLL",
    "PSAPI.DLL",
    "RPCRT4.DLL",
    "SECUR32.DLL",
    "SHELL32.DLL",
    "SHLWAPI.DLL",
    "USP10.DLL",
    "USER32.DLL",
    "USERENV.DLL",
    "WINMM.DLL",
    "WSOCK32.DLL",
    "WS2_32.DLL"
)

$queue = New-Object System.Collections.Generic.Queue[string]
$queue.Enqueue((Join-Path $OutDir "openarc.exe"))
$queue.Enqueue((Join-Path $OutDir "openarc_bpg.dll"))
$queue.Enqueue((Join-Path $OutDir "openarc_ffmpeg.dll"))

$seenFiles = New-Object System.Collections.Generic.HashSet[string]([StringComparer]::OrdinalIgnoreCase)
$missing = New-Object System.Collections.Generic.HashSet[string]([StringComparer]::OrdinalIgnoreCase)

Write-Host "Resolving DLL dependency closure..." -ForegroundColor Green
while ($queue.Count -gt 0) {
    $current = $queue.Dequeue()
    if (-not (Test-Path $current)) { continue }
    if (-not $seenFiles.Add($current)) { continue }

    $imports = Get-DllImports -ObjdumpExe $objdump -BinaryPath $current
    foreach ($dep in $imports) {
        $upper = $dep.ToUpperInvariant()
        if ($upper.StartsWith("API-MS-WIN-") -or $upper.StartsWith("EXT-MS-WIN-")) {
            continue
        }
        if ($systemDlls -contains $upper) {
            continue
        }

        $stagedDep = Join-Path $OutDir $dep
        if (Test-Path $stagedDep) {
            $queue.Enqueue($stagedDep)
            continue
        }

        $found = $null
        foreach ($dir in $searchDirs) {
            $candidate = Join-Path $dir $dep
            if (Test-Path $candidate) {
                $found = $candidate
                break
            }
        }

        if ($found) {
            Copy-Item $found $stagedDep -Force
            Write-Host "  + $dep" -ForegroundColor DarkGray
            $queue.Enqueue($stagedDep)
        } else {
            [void]$missing.Add($dep)
        }
    }
}

Write-Host ""
Write-Host "=== CLI Runtime Ready ===" -ForegroundColor Green
Write-Host "Folder: $OutDir" -ForegroundColor Cyan
$count = (Get-ChildItem -Path $OutDir -File | Measure-Object).Count
Write-Host "Files staged: $count" -ForegroundColor Gray

if ($missing.Count -gt 0) {
    Write-Host ""
    Write-Host "Missing DLLs (not copied):" -ForegroundColor Yellow
    $missing | Sort-Object | ForEach-Object { Write-Host "  - $_" -ForegroundColor Yellow }
    Write-Host "If these are system DLLs on your machine, this may still run." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "ArcMax note: CLI does not require an arcmax DLL; arcmax is statically linked into openarc.exe." -ForegroundColor Gray
