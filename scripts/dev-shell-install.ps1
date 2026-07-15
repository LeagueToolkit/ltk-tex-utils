# Dev helper: build the CLI + handler DLL, copy them to a stage directory, and
# register the Explorer shell integration from there.
#
# Why a stage directory: once the Windows 11 modern context menu is opened,
# Explorer loads the registered IExplorerCommand DLL into its own process and
# keeps it locked. Registering straight out of target\release would then make
# every `cargo build` fail with "Access is denied" on the DLL. Staging keeps
# the build outputs unlocked; only the staged copy is ever held by Explorer,
# and this script restarts Explorer automatically when it blocks re-staging.

param(
    [string]$StageDir = (Join-Path (Split-Path $PSScriptRoot -Parent) 'stage'),
    # Skip `cargo build --release` and stage whatever is already built.
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent
$releaseDir = Join-Path $repoRoot 'target\release'
$artifacts = @('ltk-tex-utils.exe', 'ltk_tex_thumb_handler.dll')

if (-not $NoBuild) {
    Write-Host 'Building (cargo build --release)...' -ForegroundColor Cyan
    cargo build --release --manifest-path (Join-Path $repoRoot 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
}

foreach ($name in $artifacts) {
    if (-not (Test-Path -LiteralPath (Join-Path $releaseDir $name))) {
        throw "$name not found in $releaseDir (build it first or drop -NoBuild)"
    }
}

if (-not (Test-Path -LiteralPath $StageDir)) {
    New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
}

# Explorer holds the staged DLL after the modern menu has been used; restart it
# once if the copy is blocked, then retry.
function Copy-Artifacts {
    foreach ($name in $artifacts) {
        Copy-Item -LiteralPath (Join-Path $releaseDir $name) -Destination $StageDir -Force
    }
}

try {
    Copy-Artifacts
} catch [System.IO.IOException], [System.UnauthorizedAccessException] {
    Write-Host 'Stage files are locked by Explorer; restarting it...' -ForegroundColor Yellow
    taskkill /f /im explorer.exe | Out-Null
    Start-Sleep -Seconds 2
    try {
        Copy-Artifacts
    } finally {
        Start-Process explorer.exe
    }
}

Write-Host "Staged to $StageDir" -ForegroundColor Green
& (Join-Path $StageDir 'ltk-tex-utils.exe') shell install
exit $LASTEXITCODE
