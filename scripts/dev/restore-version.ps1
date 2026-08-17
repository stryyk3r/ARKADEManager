# Script to restore version to 2.0.2
# Usage: .\restore-version.ps1

Write-Host "Restoring version to 2.0.2..." -ForegroundColor Yellow

# Restore to 2.0.2
(Get-Content "src-tauri/Cargo.toml") -replace "version = `".*`"", "version = `"2.0.2`"" | Set-Content "src-tauri/Cargo.toml"
$tauriConfig = Get-Content "src-tauri/tauri.conf.json" | ConvertFrom-Json
$tauriConfig.version = "2.0.2"
$tauriConfig | ConvertTo-Json -Depth 10 | Set-Content "src-tauri/tauri.conf.json"
$packageJson = Get-Content "package.json" | ConvertFrom-Json
$packageJson.version = "2.0.2"
$packageJson | ConvertTo-Json -Depth 10 | Set-Content "package.json"

Write-Host "Version restored to 2.0.2!" -ForegroundColor Green
