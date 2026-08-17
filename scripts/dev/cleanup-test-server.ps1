# Cleanup Test Server
# This script removes the test server directory structure

$testServerRoot = "C:\ArkServers\TestServer"

Write-Host "Removing test server structure..." -ForegroundColor Cyan

if (Test-Path $testServerRoot) {
    Remove-Item -Path $testServerRoot -Recurse -Force
    Write-Host "Test server removed successfully!" -ForegroundColor Green
} else {
    Write-Host "Test server directory not found. Nothing to clean up." -ForegroundColor Yellow
}
