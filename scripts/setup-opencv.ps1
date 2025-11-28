# OpenCV Setup Script for Realtr Development
# Run this script as Administrator

param(
    [string]$OpenCVVersion = "4.10.0"
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  OpenCV Setup for Realtr" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if running as admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "WARNING: Not running as Administrator. Some steps may fail." -ForegroundColor Yellow
    Write-Host "Consider re-running with: Start-Process powershell -Verb runAs -ArgumentList '-File', '$PSCommandPath'" -ForegroundColor Yellow
    Write-Host ""
}

# Step 1: Check for Chocolatey
Write-Host "[1/5] Checking for Chocolatey..." -ForegroundColor Green
if (Get-Command choco -ErrorAction SilentlyContinue) {
    Write-Host "  Chocolatey is installed" -ForegroundColor Gray
} else {
    Write-Host "  Installing Chocolatey..." -ForegroundColor Yellow
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
}

# Step 2: Install LLVM (required for opencv-rs bindings)
Write-Host "[2/5] Installing LLVM..." -ForegroundColor Green
if (Get-Command clang -ErrorAction SilentlyContinue) {
    Write-Host "  LLVM/Clang is already installed" -ForegroundColor Gray
} else {
    choco install llvm -y
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
}

# Step 3: Install OpenCV
Write-Host "[3/5] Installing OpenCV..." -ForegroundColor Green
$opencvPath = "C:\tools\opencv"
if (Test-Path "$opencvPath\build\x64\vc16\bin\opencv_world4100.dll") {
    Write-Host "  OpenCV is already installed at $opencvPath" -ForegroundColor Gray
} else {
    choco install opencv -y
    # Refresh PATH
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
}

# Step 4: Set Environment Variables
Write-Host "[4/5] Setting environment variables..." -ForegroundColor Green

# Find OpenCV installation
$possiblePaths = @(
    "C:\tools\opencv",
    "C:\opencv",
    "$env:USERPROFILE\opencv"
)

$opencvRoot = $null
foreach ($path in $possiblePaths) {
    if (Test-Path "$path\build") {
        $opencvRoot = $path
        break
    }
}

if ($opencvRoot) {
    Write-Host "  Found OpenCV at: $opencvRoot" -ForegroundColor Gray

    # Find the correct VC version folder
    $vcVersions = @("vc16", "vc15", "vc14")
    $vcPath = $null
    foreach ($vc in $vcVersions) {
        if (Test-Path "$opencvRoot\build\x64\$vc") {
            $vcPath = "$opencvRoot\build\x64\$vc"
            break
        }
    }

    if ($vcPath) {
        # Set environment variables
        [Environment]::SetEnvironmentVariable("OPENCV_DIR", "$opencvRoot\build", "User")
        [Environment]::SetEnvironmentVariable("OPENCV_INCLUDE_PATHS", "$opencvRoot\build\include", "User")
        [Environment]::SetEnvironmentVariable("OPENCV_LINK_PATHS", "$vcPath\lib", "User")

        # Find the opencv_world DLL name
        $worldDll = Get-ChildItem "$vcPath\bin" -Filter "opencv_world*.dll" | Select-Object -First 1
        if ($worldDll) {
            $libName = $worldDll.BaseName
            [Environment]::SetEnvironmentVariable("OPENCV_LINK_LIBS", $libName, "User")
            Write-Host "  Set OPENCV_LINK_LIBS=$libName" -ForegroundColor Gray
        }

        # Add to PATH
        $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($currentPath -notlike "*$vcPath\bin*") {
            [Environment]::SetEnvironmentVariable("Path", "$currentPath;$vcPath\bin", "User")
            Write-Host "  Added $vcPath\bin to PATH" -ForegroundColor Gray
        }

        # Update current session
        $env:OPENCV_DIR = "$opencvRoot\build"
        $env:OPENCV_INCLUDE_PATHS = "$opencvRoot\build\include"
        $env:OPENCV_LINK_PATHS = "$vcPath\lib"
        $env:Path = "$env:Path;$vcPath\bin"
    }
} else {
    Write-Host "  ERROR: Could not find OpenCV installation" -ForegroundColor Red
    exit 1
}

# Step 5: Copy DLLs to project
Write-Host "[5/5] Copying OpenCV DLLs to project..." -ForegroundColor Green

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$dllDestDir = Join-Path $projectRoot "src-tauri\opencv"

if (-not (Test-Path $dllDestDir)) {
    New-Item -ItemType Directory -Path $dllDestDir | Out-Null
}

# Copy all required DLLs
$dllsToCopy = @(
    "opencv_world*.dll"
)

foreach ($pattern in $dllsToCopy) {
    $dlls = Get-ChildItem "$vcPath\bin" -Filter $pattern
    foreach ($dll in $dlls) {
        Copy-Item $dll.FullName -Destination $dllDestDir -Force
        Write-Host "  Copied: $($dll.Name)" -ForegroundColor Gray
    }
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  Setup Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. RESTART your terminal/IDE to pick up new environment variables" -ForegroundColor White
Write-Host "2. Run: cargo check --manifest-path src-tauri/Cargo.toml" -ForegroundColor White
Write-Host "3. If it works, run: npm run tauri dev" -ForegroundColor White
Write-Host ""
Write-Host "DLLs have been copied to: $dllDestDir" -ForegroundColor Cyan
Write-Host "These will be bundled with your app automatically." -ForegroundColor Cyan
