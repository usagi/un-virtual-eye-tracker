param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")),
    [string]$Version,
    [string]$OutputDir,
    [switch]$SkipBuild,
    [switch]$KeepStaging
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-DefaultVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRootPath
    )

    $tauriConfigPath = Join-Path $RepoRootPath "apps/desktop/src-tauri/tauri.conf.json"
    if (-not (Test-Path $tauriConfigPath)) {
        throw "tauri config not found: $tauriConfigPath"
    }

    $tauriConfig = Get-Content -Path $tauriConfigPath -Raw | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($tauriConfig.version)) {
        throw "version is missing in $tauriConfigPath"
    }

    return [string]$tauriConfig.version
}

$RepoRoot = (Resolve-Path $RepoRoot).Path

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-DefaultVersion -RepoRootPath $RepoRoot
}

$packageName = "unvet-$Version"
$releaseDir = Join-Path $RepoRoot "target/release"
$stagingRoot = Join-Path $releaseDir "package"
$stagingDir = Join-Path $stagingRoot $packageName

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $RepoRoot "release-packages"
}
$OutputDir = (Resolve-Path -LiteralPath (New-Item -ItemType Directory -Path $OutputDir -Force)).Path
$zipPath = Join-Path $OutputDir "$packageName.zip"

if (-not $SkipBuild) {
    Push-Location $RepoRoot
    try {
        cargo build --release -p unvet-desktop -p unvet-cli
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

$requiredReleaseFiles = @(
    "unvet-desktop.exe",
    "unvet-cli.exe",
    "unvet_desktop_lib.dll",
    "NPClient64.dll",
    "NPClient.dll",
    "TrackIR.exe",
    "unvet-uninstall-compatible-layers.exe"
)

$optionalRootFiles = @(
    "README.md",
    "LICENSE",
    "THIRD_PARTY_NOTICES.md"
)

if (Test-Path $stagingDir) {
    Remove-Item -Path $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

$missing = @()
foreach ($fileName in $requiredReleaseFiles) {
    $sourcePath = Join-Path $releaseDir $fileName
    if (-not (Test-Path $sourcePath)) {
        $missing += $sourcePath
        continue
    }

    Copy-Item -Path $sourcePath -Destination (Join-Path $stagingDir $fileName) -Force
}

if ($missing.Count -gt 0) {
    $joined = ($missing -join [Environment]::NewLine)
    throw "required release artifacts are missing:`n$joined"
}

foreach ($fileName in $optionalRootFiles) {
    $sourcePath = Join-Path $RepoRoot $fileName
    if (Test-Path $sourcePath) {
        Copy-Item -Path $sourcePath -Destination (Join-Path $stagingDir $fileName) -Force
    }
}

if (Test-Path $zipPath) {
    Remove-Item -Path $zipPath -Force
}

Push-Location $stagingRoot
try {
    Compress-Archive -Path $packageName -DestinationPath $zipPath -Force
}
finally {
    Pop-Location
}

if (-not $KeepStaging) {
    Remove-Item -Path $stagingDir -Recurse -Force

    if ((Test-Path $stagingRoot) -and -not (Get-ChildItem -Path $stagingRoot -Force | Select-Object -First 1)) {
        Remove-Item -Path $stagingRoot -Force
    }
}

$zipInfo = Get-Item -Path $zipPath
Write-Host "Release package created:" -ForegroundColor Green
Write-Host ("- Path: {0}" -f $zipInfo.FullName)
Write-Host ("- Size: {0} bytes" -f $zipInfo.Length)
Write-Host ("- Updated: {0}" -f $zipInfo.LastWriteTime)

Write-Output ("PACKAGE_PATH={0}" -f $zipInfo.FullName)
