# Deprecated: the thumbnail-handler DLL now ships with the main installer, and
# registration moved into the CLI (`ltk-tex-utils handler install`). This stub
# only exists so cached copies of the old one-liner point people at the new flow.

Write-Host "This script is deprecated." -ForegroundColor Yellow
Write-Host ""
Write-Host "Install ltk-tex-utils (this also downloads the thumbnail-handler DLL):" -ForegroundColor Cyan
Write-Host "  iwr -useb https://raw.githubusercontent.com/LeagueToolkit/ltk-tex-utils/main/scripts/install-windows.ps1 | iex"
Write-Host ""
Write-Host "Then register the handler (elevates via UAC when needed):" -ForegroundColor Cyan
Write-Host "  ltk-tex-utils handler install"

exit 1
