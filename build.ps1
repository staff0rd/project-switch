#!/usr/bin/env pwsh

if ($PSScriptRoot -match '^\\\\wsl(\.localhost|\$)\\') {
    Write-Error "Refusing to build from a WSL path ($PSScriptRoot). Run build.ps1 from a Windows checkout."
    exit 1
}

$env:BUILD_TARGET = "windows"

$stagingRoot = Join-Path $PSScriptRoot "bin/.staging"
$stagedDir = Join-Path $stagingRoot "windows"
$binDir = Join-Path $PSScriptRoot "bin/windows"

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

$stagedFiles = Get-ChildItem -Path $stagedDir -File -ErrorAction SilentlyContinue
if (-not $stagedFiles) {
    Write-Error "Build produced no files in $stagedDir"
    exit 1
}

Write-Host "Build completed successfully!"

# Old processes must be gone before the new hotkey starts, or its trampoline copy into %LOCALAPPDATA% silently fails
Write-Host "Stopping any running project-switch processes..."
$running = Get-Process -Name "project-switch", "project-switch-hotkey" -ErrorAction SilentlyContinue
$running | Stop-Process -Force -ErrorAction SilentlyContinue
$running | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
Write-Host "Processes stopped."

# Replace files rather than the directory (another profile may hold a directory handle)
Write-Host "Updating bin/windows..."
$installFailed = $false
try {
    New-Item -ItemType Directory -Path $binDir -Force -ErrorAction Stop | Out-Null
    $stagedNames = $stagedFiles | ForEach-Object { $_.Name }
    Get-ChildItem -Path $binDir -File | Where-Object { $stagedNames -notcontains $_.Name } | Remove-Item -Force -ErrorAction Stop
    $stagedFiles | Copy-Item -Destination $binDir -Force -ErrorAction Stop
    Remove-Item -Path $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "bin/windows updated."
}
catch {
    Write-Error "Updating bin/windows failed: $($_.Exception.Message)"
    $installFailed = $true
}

$hotkeyPath = Join-Path $binDir "project-switch-hotkey.exe"
if (Test-Path $hotkeyPath) {
    Write-Host "Starting project-switch-hotkey..."
    Start-Process -FilePath $hotkeyPath
    Write-Host "project-switch-hotkey started."
}

if ($installFailed) {
    exit 1
}
