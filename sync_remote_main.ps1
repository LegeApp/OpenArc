param(
    [string]$Remote = "origin",
    [string]$Branch = "main",
    [string]$Whitelist = ".sync-whitelist",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Args
    )

    & git @Args
    if ($LASTEXITCODE -ne 0) {
        throw "git command failed: git $($Args -join ' ')"
    }
}

$repoRoot = (& git rev-parse --show-toplevel).Trim()
if (-not $repoRoot) {
    throw "Unable to determine repository root."
}

Set-Location -LiteralPath $repoRoot

$whitelistPath = if ([System.IO.Path]::IsPathRooted($Whitelist)) {
    $Whitelist
} else {
    Join-Path $repoRoot $Whitelist
}

if (-not (Test-Path -LiteralPath $whitelistPath)) {
    throw "Whitelist file not found: $whitelistPath"
}

$remoteUrl = (& git remote get-url $Remote).Trim()
if (-not $remoteUrl) {
    throw "Unable to resolve remote URL for '$Remote'."
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("openarc-sync-" + [Guid]::NewGuid().ToString("N"))
$exportDir = Join-Path $tempRoot "export"

try {
    New-Item -ItemType Directory -Path $exportDir -Force | Out-Null

    $paths = Get-Content -LiteralPath $whitelistPath |
        ForEach-Object { $_.Trim() } |
        Where-Object { $_ -and -not $_.StartsWith("#") }

    Write-Host "Exporting paths from $Whitelist ..."

    foreach ($relPath in $paths) {
        $srcPath = Join-Path $repoRoot $relPath
        if (-not (Test-Path -LiteralPath $srcPath)) {
            Write-Warning "Skipping missing path: $relPath"
            continue
        }

        $destPath = Join-Path $exportDir $relPath
        $destParent = Split-Path -Parent $destPath
        if ($destParent) {
            New-Item -ItemType Directory -Path $destParent -Force | Out-Null
        }

        Copy-Item -LiteralPath $srcPath -Destination $destPath -Recurse -Force
    }

    $excludeDirNames = @("target", "Release", "Debug", ".git")

    $allDirs = @(Get-ChildItem -LiteralPath $exportDir -Directory -Recurse -Force | Sort-Object { $_.FullName.Length } -Descending)
    foreach ($dir in $allDirs) {
        $isNameExcluded = $excludeDirNames -contains $dir.Name
        $isCliRuntime = $dir.FullName -like "*${([IO.Path]::DirectorySeparatorChar)}dist${([IO.Path]::DirectorySeparatorChar)}cli-runtime"
        if ($isNameExcluded -or $isCliRuntime) {
            Remove-Item -LiteralPath $dir.FullName -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    $excludeFilePatterns = @("*.log", "*.tmp", "*.obj", "*.pdb")
    foreach ($pattern in $excludeFilePatterns) {
        Get-ChildItem -LiteralPath $exportDir -File -Recurse -Filter $pattern -Force |
            Remove-Item -Force -ErrorAction SilentlyContinue
    }

    Set-Location -LiteralPath $exportDir

    Invoke-Git -Args @("init", "-q")
    Invoke-Git -Args @("config", "user.name", "OpenArc Sync Bot")
    Invoke-Git -Args @("config", "user.email", "sync-bot@local")
    Invoke-Git -Args @("checkout", "-b", $Branch)

    Invoke-Git -Args @("add", "-A")

    & git diff --cached --quiet
    if ($LASTEXITCODE -eq 0) {
        Write-Host "No files matched whitelist or nothing changed."
        exit 0
    }

    $stamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd HH:mm:ssZ")
    Invoke-Git -Args @("commit", "-m", "Sync OpenArc snapshot ($stamp)")

    Invoke-Git -Args @("remote", "add", $Remote, $remoteUrl)

    Write-Host "Pushing snapshot to $Remote/$Branch (no pull performed)..."

    $remoteHeadLine = (& git ls-remote --heads $Remote $Branch | Select-Object -First 1).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read remote head for $Remote/$Branch"
    }

    $pushArgs = @("push")
    if ($remoteHeadLine) {
        $remoteHeadSha = ($remoteHeadLine -split "\s+")[0]
        $pushArgs += "--force-with-lease=refs/heads/$Branch`:$remoteHeadSha"
    } else {
        # Branch does not exist remotely yet.
        $pushArgs += "--force"
    }

    if ($DryRun.IsPresent) {
        $pushArgs += "--dry-run"
    }
    $pushArgs += @($Remote, "$Branch`:$Branch")

    Invoke-Git -Args $pushArgs

    Write-Host "Done."
}
finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
