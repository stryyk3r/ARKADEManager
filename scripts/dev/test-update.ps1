# Script to test update functionality by temporarily lowering version
# Usage: .\test-update.ps1

Write-Host "Testing Update Functionality" -ForegroundColor Cyan
Write-Host "============================" -ForegroundColor Cyan
Write-Host ""

# Backup current version
$cargoVersion = (Get-Content "src-tauri/Cargo.toml" | Select-String "version = ").ToString()
$tauriVersion = (Get-Content "src-tauri/tauri.conf.json" | ConvertFrom-Json).version
$packageVersion = (Get-Content "package.json" | ConvertFrom-Json).version

Write-Host "Current versions:" -ForegroundColor Yellow
Write-Host "  Cargo.toml: $cargoVersion"
Write-Host "  tauri.conf.json: $tauriVersion"
Write-Host "  package.json: $packageVersion"
Write-Host ""

# Ask for test version
$testVersion = Read-Host "Enter version to test with (e.g., 2.0.1)"

Write-Host ""
Write-Host "Updating versions to $testVersion..." -ForegroundColor Yellow

# Update versions
(Get-Content "src-tauri/Cargo.toml") -replace "version = `".*`"", "version = `"$testVersion`"" | Set-Content "src-tauri/Cargo.toml"
$tauriConfig = Get-Content "src-tauri/tauri.conf.json" | ConvertFrom-Json
$tauriConfig.version = $testVersion
$tauriConfig | ConvertTo-Json -Depth 10 | Set-Content "src-tauri/tauri.conf.json"
$packageJson = Get-Content "package.json" | ConvertFrom-Json
$packageJson.version = $testVersion
$packageJson | ConvertTo-Json -Depth 10 | Set-Content "package.json"

Write-Host "Versions updated!" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "1. Build the app: npm run tauri build"
Write-Host "2. Run the built app from: src-tauri\target\release\arkade-manager.exe"
Write-Host "3. The app should detect a newer version if one exists on GitHub"
Write-Host ""
Write-Host "To restore versions, run: .\restore-version.ps1" -ForegroundColor Yellow
