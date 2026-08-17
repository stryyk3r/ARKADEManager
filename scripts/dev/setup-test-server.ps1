# Setup Test Server for Plugin Toggle Testing
# This script creates a test server directory structure with plugin folders

$testServerRoot = "C:\ArkServers\TestServer"
$pluginsPath = Join-Path $testServerRoot "ShooterGame\Binaries\Win64\ArkApi\Plugins"

Write-Host "Creating test server structure..." -ForegroundColor Cyan

# Create directory structure
New-Item -ItemType Directory -Path $pluginsPath -Force | Out-Null

Write-Host "Created plugins directory: $pluginsPath" -ForegroundColor Green

# Create test plugin folders (some enabled, some disabled)
$testPlugins = @(
    "PluginA",
    "PluginB",
    "PluginC_OFF",
    "PluginD",
    "PluginE_OFF",
    "TestPlugin",
    "AnotherPlugin_OFF"
)

Write-Host "`nCreating test plugin folders..." -ForegroundColor Cyan

foreach ($plugin in $testPlugins) {
    $pluginPath = Join-Path $pluginsPath $plugin
    New-Item -ItemType Directory -Path $pluginPath -Force | Out-Null
    
    # Create a dummy file in each folder so they're not empty
    $dummyFile = Join-Path $pluginPath "dummy.txt"
    Set-Content -Path $dummyFile -Value "Test plugin folder: $plugin"
    
    $status = if ($plugin.EndsWith("_OFF")) { "DISABLED" } else { "ENABLED" }
    Write-Host "  Created: $plugin ($status)" -ForegroundColor Yellow
}

Write-Host "`nTest server structure created successfully!" -ForegroundColor Green
Write-Host "Server Root: $testServerRoot" -ForegroundColor Cyan
Write-Host "Plugins Path: $pluginsPath" -ForegroundColor Cyan
Write-Host "`nTo test:" -ForegroundColor Yellow
Write-Host "1. Add a backup job in ARKADE Manager with root directory: $testServerRoot" -ForegroundColor White
Write-Host "2. Go to Plugin Toggle tab" -ForegroundColor White
Write-Host "3. Select the test server from the dropdown" -ForegroundColor White
Write-Host "4. Test toggling plugins!" -ForegroundColor White
