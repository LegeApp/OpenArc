param(
    [switch]$Debug,
    [switch]$Release,
    [switch]$SkipCodecs,
    [switch]$SkipRuntimeStage,
    [string]$Target = "x86_64-pc-windows-gnu"
)

if ($Debug -and $Release) {
    throw "Use either -Debug or -Release, not both."
}

$RustRelease = $Release -or (-not $Debug)
$ErrorActionPreference = "Stop"

Write-Host "=== OpenArc Windows CLI Build ===" -ForegroundColor Cyan
Write-Host "Target: $Target" -ForegroundColor Gray
Write-Host "Mode: $(if ($RustRelease) { 'Release' } else { 'Debug' })" -ForegroundColor Gray
Write-Host "GUI build: disabled" -ForegroundColor Gray
Write-Host ""

if (-not $SkipCodecs) {
    Write-Host "[1/3] Building native codec dependencies..." -ForegroundColor Green

    Push-Location "$PSScriptRoot\native\BPG\libbpg-0.9.8"
    try {
        & "$PSScriptRoot\native\BPG\libbpg-0.9.8\build_complete.bat"
        if ($LASTEXITCODE -ne 0) {
            throw "BPG codec build failed"
        }
    } finally {
        Pop-Location
    }

    $buildCodecsScript = Join-Path $PSScriptRoot "crates\arcmax\build_codecs.ps1"
    if (-not (Test-Path $buildCodecsScript)) {
        throw "ArcMax codec build script not found at $buildCodecsScript"
    }

    & powershell -NoProfile -ExecutionPolicy Bypass -File $buildCodecsScript
    if ($LASTEXITCODE -ne 0) {
        throw "ArcMax codec build failed"
    }
} else {
    Write-Host "[1/3] Skipping native codec dependencies (--SkipCodecs)." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "[2/3] Building openarc CLI..." -ForegroundColor Green
Push-Location $PSScriptRoot
try {
    $cargoArgs = @("build", "-p", "openarc", "--bin", "openarc", "--target", $Target)
    if ($RustRelease) {
        $cargoArgs += "--release"
    }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed"
    }
} finally {
    Pop-Location
}

Write-Host ""
if (-not $SkipRuntimeStage) {
    Write-Host "[3/3] Staging Windows CLI runtime..." -ForegroundColor Green
    $runtimeArgs = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $PSScriptRoot "build-cli-runtime.ps1"),
        "-Target", $Target,
        "-SkipBuild"
    )
    if ($RustRelease) {
        # build-cli-runtime.ps1 always stages the release binary; enforce that here.
    } elseif ($Debug) {
        throw "Debug cargo builds are not supported with runtime staging. Re-run with -SkipRuntimeStage or release mode."
    }

    & powershell @runtimeArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Runtime staging failed"
    }
} else {
    Write-Host "[3/3] Skipping runtime staging (--SkipRuntimeStage)." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Build complete." -ForegroundColor Green
if ($RustRelease) {
    Write-Host "Binary: $PSScriptRoot\target\$Target\release\openarc.exe" -ForegroundColor Cyan
} else {
    Write-Host "Binary: $PSScriptRoot\target\$Target\debug\openarc.exe" -ForegroundColor Cyan
}
if (-not $SkipRuntimeStage) {
    Write-Host "Runtime bundle: $PSScriptRoot\dist\cli-runtime" -ForegroundColor Cyan
}
