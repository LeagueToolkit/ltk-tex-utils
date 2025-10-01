param(
    [string]$Owner = "LeagueToolkit",
    [string]$Repo  = "ltk-tex-utils",
    [string]$InstallDir = "$env:LOCALAPPDATA\LeagueToolkit\ltk-tex-utils"
)

$ErrorActionPreference = 'Stop'

Write-Host "Installing ltk-tex-utils..." -ForegroundColor Cyan

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

# Our release workflow uploads a single Windows asset named ltk-tex-utils-windows.exe
$assetName = "ltk-tex-utils-windows.exe"
$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
if (-not $asset) {
    # Fallback: find any windows exe for this project
    $asset = $release.assets | Where-Object { $_.name -match '^ltk-tex-utils-.*windows.*\.exe$' } | Select-Object -First 1
}
if (-not $asset) {
    throw "Could not find a Windows asset in the latest release."
}
$assetName = $asset.name

$exePath = Join-Path $InstallDir 'ltk-tex-utils.exe'
$tmpPath = Join-Path $env:TEMP $assetName

Write-Host "Downloading $assetName ($version)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tmpPath -UseBasicParsing

Write-Host "Placing binary into $InstallDir" -ForegroundColor Yellow
Copy-Item -LiteralPath $tmpPath -Destination $exePath -Force

# Create a shim directory so PATH is simple and stable
$binDir = Join-Path $InstallDir 'bin'
if (!(Test-Path -LiteralPath $binDir)) { New-Item -ItemType Directory -Path $binDir | Out-Null }

# Ensure the executable exists
if (!(Test-Path -LiteralPath $exePath)) {
    throw "ltk-tex-utils.exe not found after download: $exePath"
}

# Place a thin cmd shim in bin to avoid spaces in paths and simplify PATH updates
$shimCmd = @"
@echo off
""$exePath"" %*
"@
Set-Content -LiteralPath (Join-Path $binDir 'ltk-tex-utils.cmd') -Value $shimCmd -Encoding Ascii -Force

# Add to user PATH if missing
$currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not ($currentPath -split ';' | Where-Object { $_ -eq $binDir })) {
    $newPath = if ([string]::IsNullOrEmpty($currentPath)) { $binDir } else { "$currentPath;$binDir" }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Host "Added to PATH (User): $binDir" -ForegroundColor Green
} else {
    Write-Host "PATH already contains: $binDir" -ForegroundColor Green
}

Write-Host "Installed ltk-tex-utils $version to $InstallDir" -ForegroundColor Green
Write-Host "Open a new terminal and run: ltk-tex-utils --help" -ForegroundColor Cyan


