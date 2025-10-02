#Requires -RunAsAdministrator

param(
    [string]$Owner = "LeagueToolkit",
    [string]$Repo  = "ltk-tex-utils",
    [string]$InstallDir = "$env:ProgramFiles\LeagueToolkit\ltk-tex-thumb-handler"
)

$ErrorActionPreference = 'Stop'

Write-Host "Installing ltk-tex-thumb-handler (Windows Explorer thumbnail provider)..." -ForegroundColor Cyan
Write-Host "This script requires administrator privileges to register the COM DLL." -ForegroundColor Yellow

if (!(Test-Path -LiteralPath $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Get latest release metadata
$releaseApi = "https://api.github.com/repos/$Owner/$Repo/releases/latest"
try {
    $release = Invoke-RestMethod -Uri $releaseApi -Headers @{ 'User-Agent' = 'ltk-tex-utils-installer' }
} catch {
    throw "Failed to query GitHub releases: $($_.Exception.Message)"
}

$tag = $release.tag_name
# Extract the first semantic version (handles tags like "v0.1.1")
$match = [regex]::Match($tag, '\d+\.\d+\.\d+([\-\+][A-Za-z0-9\.-]+)?')
$version = if ($match.Success) { $match.Value } else { $tag.TrimStart('v') }

# Find the thumbnail handler DLL asset
$assetName = "ltk-tex-thumb-handler.dll"
$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
if (-not $asset) {
    throw "Could not find $assetName in the latest release. Make sure you're using a release that includes the thumbnail handler."
}

$dllPath = Join-Path $InstallDir 'ltk_tex_thumb_handler.dll'
$tmpPath = Join-Path $env:TEMP $assetName

Write-Host "Downloading $assetName ($version)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tmpPath -UseBasicParsing

# Unregister old DLL if it exists
if (Test-Path -LiteralPath $dllPath) {
    Write-Host "Unregistering existing DLL..." -ForegroundColor Yellow
    try {
        & regsvr32.exe /s /u $dllPath
    } catch {
        Write-Warning "Failed to unregister old DLL: $($_.Exception.Message)"
    }
}

Write-Host "Installing DLL to $InstallDir" -ForegroundColor Yellow
Copy-Item -LiteralPath $tmpPath -Destination $dllPath -Force

# Ensure the DLL exists
if (!(Test-Path -LiteralPath $dllPath)) {
    throw "ltk_tex_thumb_handler.dll not found after download: $dllPath"
}

# Register the COM DLL
Write-Host "Registering COM DLL with Windows..." -ForegroundColor Yellow
$regResult = & regsvr32.exe /s $dllPath
if ($LASTEXITCODE -ne 0) {
    throw "Failed to register DLL. regsvr32 returned exit code: $LASTEXITCODE"
}

Write-Host "Successfully installed and registered ltk-tex-thumb-handler $version" -ForegroundColor Green
Write-Host "Windows Explorer will now show thumbnails for .tex files." -ForegroundColor Cyan
Write-Host ""
Write-Host "Note: You may need to restart Windows Explorer or your computer for thumbnails to appear." -ForegroundColor Yellow
Write-Host "To restart Explorer: Task Manager > Windows Explorer > Restart" -ForegroundColor Gray

# Clean up temp file
Remove-Item -LiteralPath $tmpPath -Force -ErrorAction SilentlyContinue

