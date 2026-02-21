#!/usr/bin/env pwsh

# Kill any running project-switch.exe processes
Write-Host "Stopping any running project-switch.exe processes..."
Get-Process -Name "project-switch", "project-switch-hotkey" -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Host "Processes stopped."

# Delete the bin folder if it exists
Write-Host "Removing bin folder..."
if (Test-Path "bin") {
    try {
        Remove-Item -Path "bin" -Recurse -Force -ErrorAction Stop
        Write-Host "bin folder removed."
    }
    catch {
        Write-Error "Failed to remove bin folder. Build may be incomplete."
        Write-Error $_.Exception.Message
        exit 1
    }
}
else {
    Write-Host "bin folder does not exist."
}

# Only build for Windows
$env:BUILD_TARGET = "windows"

# Force rebuild of Docker container and run build service
Write-Host "Building Docker container and running build service..."

# Capture output and only show on failure
$buildOutput = docker compose build 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "Docker build failed:"
    Write-Host $buildOutput
    exit 1
}

$runOutput = docker compose run --rm build 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "Docker build service failed:"
    Write-Host $runOutput
    exit 1
}

Write-Host "Build completed successfully!"

# Restart the hotkey listener
$hotkeyPath = Join-Path $PSScriptRoot "bin/windows/project-switch-hotkey.exe"
if (Test-Path $hotkeyPath) {
    Write-Host "Starting project-switch-hotkey..."
    Start-Process -FilePath $hotkeyPath
    Write-Host "project-switch-hotkey started."
}