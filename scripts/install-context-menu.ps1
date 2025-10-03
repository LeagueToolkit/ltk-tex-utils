# League Toolkit Context Menu Integration
# Adds context menu options for encoding/decoding .tex texture files

param(
    [Parameter(Position=0)]
    [ValidateSet("install", "uninstall")]
    [string]$Action = "install",
    
    [string]$ExecutablePath = "$env:LOCALAPPDATA\LeagueToolkit\ltk-tex-utils\ltk-tex-utils.exe",
    
    [switch]$AllUsers
)

$ErrorActionPreference = 'Stop'

# Registry root (HKCU for current user, HKLM for all users)
$registryRoot = if ($AllUsers) {
    $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        throw "Installing for all users requires administrator privileges."
    }
    "HKLM:\SOFTWARE\Classes"
} else {
    "HKCU:\Software\Classes"
}

#region Helper Functions

function New-ContextMenuCommand {
    param(
        [string]$Extension,
        [string]$CommandId,
        [string]$DisplayName,
        [string]$Command
    )
    
    $extKey = "$registryRoot\$Extension"
    $shellKey = "$extKey\shell\$CommandId"
    $commandKey = "$shellKey\command"
    
    # Ensure extension key exists
    if (-not (Test-Path $extKey)) {
        New-Item -Path $extKey -Force | Out-Null
    }
    
    # Create menu item
    New-Item -Path $shellKey -Force | Out-Null
    Set-ItemProperty -Path $shellKey -Name "(Default)" -Value $DisplayName -Type String
    Set-ItemProperty -Path $shellKey -Name "Icon" -Value "`"$ExecutablePath`",0" -Type String
    
    # Create command
    New-Item -Path $commandKey -Force | Out-Null
    Set-ItemProperty -Path $commandKey -Name "(Default)" -Value $Command -Type String
}

function Remove-ContextMenuCommand {
    param(
        [string]$Extension,
        [string]$CommandId
    )
    
    $shellKey = "$registryRoot\$Extension\shell\$CommandId"
    if (Test-Path $shellKey) {
        Remove-Item -Path $shellKey -Recurse -Force
        return $true
    }
    return $false
}

function Get-PowerShellCommand {
    param(
        [string]$Operation,
        [string]$OutputExtension = "",
        [string]$Format = "bc3"
    )
    
    if ($Operation -eq "decode") {
        return "powershell.exe -NoProfile -WindowStyle Hidden -Command `"& '$ExecutablePath' decode -i '%1' -o ([IO.Path]::ChangeExtension('%1', '$OutputExtension'))`""
    }
    else {
        return "powershell.exe -NoProfile -WindowStyle Hidden -Command `"& '$ExecutablePath' encode -i '%1' -o ([IO.Path]::ChangeExtension('%1', 'tex')) -f $Format -m true --mipmap-filter lanczos3`""
    }
}

function Restart-Explorer {
    Write-Host "Refreshing Windows Explorer..." -ForegroundColor Yellow
    try {
        Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 500
        Start-Process explorer
        Write-Host "Explorer refreshed successfully." -ForegroundColor Green
    }
    catch {
        Write-Warning "Could not restart Explorer automatically. Please restart it manually or log off/on."
    }
}

#endregion

#region Install

function Install-ContextMenus {
    Write-Host "Installing League Toolkit context menus..." -ForegroundColor Cyan
    Write-Host ""
    
    # Verify executable exists
    if (-not (Test-Path -LiteralPath $ExecutablePath)) {
        Write-Warning "ltk-tex-utils not found at: $ExecutablePath"
        Write-Host "Install it first: iwr -useb https://raw.githubusercontent.com/LeagueToolkit/ltk-tex-utils/main/scripts/install-windows.ps1 | iex" -ForegroundColor Gray
        $response = Read-Host "Continue anyway? (y/N)"
        if ($response -ne 'y' -and $response -ne 'Y') {
            throw "Installation cancelled."
        }
    }
    
    # === .tex files: Decode to PNG ===
    Write-Host "Configuring .tex context menu..." -ForegroundColor Yellow
    
    New-ContextMenuCommand -Extension ".tex" -CommandId "ltk.decode.png" `
        -DisplayName "Decode to PNG" `
        -Command (Get-PowerShellCommand -Operation "decode" -OutputExtension "png")
    
    # === .png files: Encode to .tex ===
    Write-Host "Configuring .png context menu..." -ForegroundColor Yellow
    
    New-ContextMenuCommand -Extension ".png" -CommandId "ltk.encode" `
        -DisplayName "Encode to .tex" `
        -Command (Get-PowerShellCommand -Operation "encode")
    
    # === .dds files: Encode to .tex ===
    Write-Host "Configuring .dds context menu..." -ForegroundColor Yellow
    
    New-ContextMenuCommand -Extension ".dds" -CommandId "ltk.encode" `
        -DisplayName "Encode to .tex" `
        -Command (Get-PowerShellCommand -Operation "encode")
    
    Write-Host ""
    Write-Host "Successfully installed context menus!" -ForegroundColor Green
    Write-Host ""
    
    Restart-Explorer
    
    Write-Host ""
    Write-Host "Usage:" -ForegroundColor Cyan
    Write-Host "  .tex files  -> Right-click -> Decode to PNG" -ForegroundColor Gray
    Write-Host "  .png/.dds   -> Right-click -> Encode to .tex" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Note: Items appear in 'Show more options' on Windows 11." -ForegroundColor Yellow
}

#endregion

#region Uninstall

function Uninstall-ContextMenus {
    Write-Host "Uninstalling League Toolkit context menus..." -ForegroundColor Cyan
    Write-Host ""
    
    $removed = $false
    
    # Remove .tex menus
    if (Remove-ContextMenuCommand -Extension ".tex" -CommandId "ltk.decode.png") {
        Write-Host "Removed: .tex -> Decode to PNG" -ForegroundColor Green
        $removed = $true
    }
    # Clean up old DDS option (no longer supported)
    if (Remove-ContextMenuCommand -Extension ".tex" -CommandId "ltk.decode.dds") {
        Write-Host "Removed: .tex -> Decode to DDS (deprecated)" -ForegroundColor Green
        $removed = $true
    }
    
    # Remove .png menu
    if (Remove-ContextMenuCommand -Extension ".png" -CommandId "ltk.encode") {
        Write-Host "Removed: .png -> Encode to .tex" -ForegroundColor Green
        $removed = $true
    }
    
    # Remove .dds menu
    if (Remove-ContextMenuCommand -Extension ".dds" -CommandId "ltk.encode") {
        Write-Host "Removed: .dds -> Encode to .tex" -ForegroundColor Green
        $removed = $true
    }
    
    Write-Host ""
    if ($removed) {
        Write-Host "Successfully uninstalled context menus." -ForegroundColor Green
        Restart-Explorer
    }
    else {
        Write-Host "No context menu entries found." -ForegroundColor Yellow
    }
}

#endregion

#region Main

try {
    switch ($Action) {
        "install"   { Install-ContextMenus }
        "uninstall" { Uninstall-ContextMenus }
    }
}
catch {
    Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

#endregion
