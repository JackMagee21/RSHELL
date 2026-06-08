$repo = "JackMagee21/RSHELL"
$installDir = "$env:USERPROFILE\RSHELL"
$exePath = "$installDir\rSHELL.exe"

Write-Host ""
Write-Host "  Installing rSHELL..." -ForegroundColor Cyan

# Fetch latest release from GitHub API
try {
    $release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
} catch {
    Write-Host "  Error: Could not reach GitHub API. Check your internet connection." -ForegroundColor Red
    exit 1
}

# Find the .exe asset
$asset = $release.assets | Where-Object { $_.name -like "*.exe" } | Select-Object -First 1
if (-not $asset) {
    Write-Host "  Error: No .exe found in the latest release." -ForegroundColor Red
    exit 1
}

# Create install directory
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

# Download the binary
Write-Host "  Downloading $($asset.name) ($([math]::Round($asset.size / 1MB, 1)) MB)..." -ForegroundColor Gray
try {
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exePath -UseBasicParsing
} catch {
    Write-Host "  Error: Download failed." -ForegroundColor Red
    exit 1
}

# Add to user PATH if not already present
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$installDir;$currentPath", "User")
    Write-Host "  Added $installDir to PATH" -ForegroundColor Gray
}

Write-Host ""
Write-Host "  Done! rSHELL $($release.tag_name) installed." -ForegroundColor Green
Write-Host "  Restart your terminal, then type: rSHELL" -ForegroundColor Green
Write-Host ""