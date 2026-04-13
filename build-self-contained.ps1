param(
    [switch]$Debug,
    [switch]$Release,
    [switch]$SkipCodecs
)

$ErrorActionPreference = "Stop"

Write-Host "build-self-contained.ps1 is now a compatibility wrapper." -ForegroundColor Yellow
Write-Host "Use build-all.ps1 as the supported Windows build entrypoint for the CLI." -ForegroundColor Yellow
Write-Host ""

$args = @()
if ($Debug) { $args += "-Debug" }
if ($Release) { $args += "-Release" }
if ($SkipCodecs) { $args += "-SkipCodecs" }

& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "build-all.ps1") @args
exit $LASTEXITCODE
