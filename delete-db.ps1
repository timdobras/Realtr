$dbPath = Join-Path $env:APPDATA "com.gumeq.realtr\properties.db"
$configPath = Join-Path $env:APPDATA "com.gumeq.realtr\config.json"

Write-Host "Checking for database files..."

if (Test-Path $dbPath) {
    Remove-Item $dbPath -Force
    Write-Host "[OK] Database deleted: $dbPath"
} else {
    Write-Host "[INFO] Database not found at: $dbPath"
}

if (Test-Path $configPath) {
    Write-Host "[INFO] Config file exists at: $configPath (keeping it for folder paths)"
} else {
    Write-Host "[INFO] Config not found"
}

Write-Host ""
Write-Host "Database reset complete. The app will create a fresh database on next launch."
