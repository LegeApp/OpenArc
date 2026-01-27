param(
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$freeArcPath = Join-Path $projectRoot "freearc_cpp_lib"
$stagingPath = Join-Path $projectRoot "codec_staging"
$buildPath = Join-Path $projectRoot "codec_build"

if (-not (Test-Path $freeArcPath)) {
    throw "FreeArc source directory not found: $freeArcPath"
}

if ($Clean) {
    if (Test-Path $stagingPath) {
        Remove-Item -Path $stagingPath -Recurse -Force
    }
    if (Test-Path $buildPath) {
        Remove-Item -Path $buildPath -Recurse -Force
    }
    Write-Host "Cleaned codec staging/build directories." -ForegroundColor Green
    return
}

function Resolve-Tool($name) {
    $tool = Get-Command $name -ErrorAction SilentlyContinue
    if (-not $tool) {
        throw "Required tool '$name' not found. Ensure MinGW-w64 (gcc/g++/ar/ranlib) is installed and in PATH."
    }
    return $tool.Source
}

$gcc = Resolve-Tool "gcc"
$gpp = Resolve-Tool "g++"
$ar = Resolve-Tool "ar"
$ranlib = Resolve-Tool "ranlib"

New-Item -ItemType Directory -Force -Path $stagingPath | Out-Null
New-Item -ItemType Directory -Force -Path $buildPath | Out-Null

# Clean previous build artifacts
Get-ChildItem -Path $stagingPath -Include *.a,*.o -File -Recurse -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem -Path $buildPath -File -Recurse -ErrorAction SilentlyContinue | Remove-Item -Force

$commonFlags = @("-O2","-fPIC","-Wall","-Wextra","-D_WIN32","-DWIN32","-DWIN32_LEAN_AND_MEAN","-DNOMINMAX","-DNDEBUG","-DWINVER=0x0601","-D_WIN32_WINNT=0x0601","-DNOVERSETCONDITIONMASK","-D__USE_MINGW_ANSI_STDIO=0")
$cxxFlags = $commonFlags + @("-std=c++11")

function Invoke-Compile {
    param(
        [string]$Compiler,
        [string[]]$Flags,
        [string]$Source,
        [string[]]$Includes,
        [string]$Output
    )

    $includeArgs = $Includes | ForEach-Object { "-I$_" }
    $cmdArgs = @($Flags + $includeArgs + @("-c", $Source, "-o", $Output))
    Write-Host "  Compiling $Source" -ForegroundColor Yellow
    & $Compiler $cmdArgs
}

function Build-Codec {
    param(
        [string]$Name,
        [string[]]$Sources,
        [string[]]$Includes,
        [string]$LibraryName
    )

    Write-Host "Building $Name codec" -ForegroundColor Cyan
    $codecBuildDir = Join-Path $buildPath $Name
    New-Item -ItemType Directory -Force -Path $codecBuildDir | Out-Null
    $objects = @()

    foreach ($source in $Sources) {
        $base = [System.IO.Path]::GetFileNameWithoutExtension($source)
        $objectPath = Join-Path $codecBuildDir "$base.o"
        $compiler = if ($source.ToLower().EndsWith(".cpp")) { $gpp } else { $gcc }
        $flags = if ($compiler -eq $gpp) { $cxxFlags } else { $commonFlags }
        Invoke-Compile -Compiler $compiler -Flags $flags -Source $source -Includes $Includes -Output $objectPath
        $objects += $objectPath
    }

    $libPath = Join-Path $stagingPath "lib$LibraryName.a"
    Write-Host "  Creating $libPath" -ForegroundColor Green
    & $ar "rcs" $libPath $objects
    & $ranlib $libPath | Out-Null
}

function Build-Common {
    $commonDir = Join-Path $buildPath "common"
    New-Item -ItemType Directory -Force -Path $commonDir | Out-Null
    $includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"))
    $sources = @(
        "Compression/Common.cpp",
        "Compression/CompressionLibrary.cpp",
        "Compression/CELS.cpp",
        "Compression/MultiThreading.cpp"
    )

    foreach ($src in $sources) {
        $sourcePath = Join-Path $freeArcPath $src
        $obj = Join-Path $commonDir ([System.IO.Path]::GetFileNameWithoutExtension($src) + ".o")
        Invoke-Compile -Compiler $gpp -Flags $cxxFlags -Source $sourcePath -Includes $includes -Output $obj
        Copy-Item -Path $obj -Destination $stagingPath -Force
    }
}

Build-Common

$codecDefinitions = @(
    @{ Name = "lzma2";     Sources = @((Join-Path $freeArcPath "Compression/LZMA2/C_LZMA.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/LZMA2")); Library = "lzma2" },
    @{ Name = "ppmd";      Sources = @((Join-Path $freeArcPath "Compression/PPMD/C_PPMD.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/PPMD")); Library = "ppmd" },
    @{ Name = "tornado";   Sources = @((Join-Path $freeArcPath "Compression/Tornado/C_Tornado.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/Tornado")); Library = "tornado" },
    @{ Name = "grzip";     Sources = @((Join-Path $freeArcPath "Compression/GRZip/C_GRZip.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/GRZip")); Library = "grzip" },
    @{ Name = "lzp";       Sources = @((Join-Path $freeArcPath "Compression/LZP/C_LZP.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/LZP")); Library = "lzp" },
    @{ Name = "delta";     Sources = @((Join-Path $freeArcPath "Compression/Delta/C_Delta.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/Delta")); Library = "delta" },
    @{ Name = "dict";      Sources = @((Join-Path $freeArcPath "Compression/Dict/C_Dict.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/Dict")); Library = "dict" },
    @{ Name = "mm";        Sources = @((Join-Path $freeArcPath "Compression/MM/C_MM.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/MM")); Library = "mm" },
    @{ Name = "rep";       Sources = @((Join-Path $freeArcPath "Compression/REP/C_REP.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/REP")); Library = "rep" },
    @{ Name = "4x4";       Sources = @((Join-Path $freeArcPath "Compression/4x4/C_4x4.cpp")); Includes = @($freeArcPath, (Join-Path $freeArcPath "Compression"), (Join-Path $freeArcPath "Compression/4x4")); Library = "4x4" }
)

foreach ($codec in $codecDefinitions) {
    Build-Codec -Name $codec.Name -Sources $codec.Sources -Includes $codec.Includes -LibraryName $codec.Library
}

# Build the FreeArc wrapper
$wrapperSource = Join-Path $freeArcPath "freearc_wrapper.cpp"
$wrapperObj = Join-Path $stagingPath "freearc_wrapper.o"
$wrapperIncludes = @(
    $freeArcPath,
    (Join-Path $freeArcPath "Compression"),
    (Join-Path $freeArcPath "Compression/LZMA2"),
    (Join-Path $freeArcPath "Compression/PPMD"),
    (Join-Path $freeArcPath "Compression/Tornado"),
    (Join-Path $freeArcPath "Compression/GRZip"),
    (Join-Path $freeArcPath "Compression/LZP"),
    (Join-Path $freeArcPath "Compression/Delta"),
    (Join-Path $freeArcPath "Compression/Dict"),
    (Join-Path $freeArcPath "Compression/MM"),
    (Join-Path $freeArcPath "Compression/REP"),
    (Join-Path $freeArcPath "Compression/4x4")
)
Invoke-Compile -Compiler $gpp -Flags $cxxFlags -Source $wrapperSource -Includes $wrapperIncludes -Output $wrapperObj

# Create combined freearc library
Write-Host "Creating combined libfreearc.a" -ForegroundColor Cyan
$allObjects = Get-ChildItem -Path $stagingPath -Filter *.o | Select-Object -ExpandProperty FullName
$combinedLib = Join-Path $stagingPath "libfreearc.a"
if ($allObjects.Count -eq 0) {
    throw "No object files were generated; cannot create libfreearc.a"
}
& $ar "rcs" $combinedLib $allObjects
& $ranlib $combinedLib | Out-Null

Write-Host "FreeArc codecs built successfully." -ForegroundColor Green
Write-Host "Staged libraries:"
Get-ChildItem -Path $stagingPath -Filter *.a | ForEach-Object { Write-Host "  $_" }
