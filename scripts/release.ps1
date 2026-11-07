# Build a release exe, but only if every test passes.
# Usage: powershell -File scripts\release.ps1
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
git config core.hooksPath .githooks

# a running pet locks the exe and makes the build fail with "Access is denied"
Get-Process pixel-pet -ErrorAction SilentlyContinue | Stop-Process -Force

cargo test
if ($LASTEXITCODE -ne 0) { Write-Host "Tests failed: not building a release." -ForegroundColor Red; exit 1 }
cargo build --release
if ($LASTEXITCODE -ne 0) { exit 1 }
Write-Host "OK: target\release\pixel-pet.exe" -ForegroundColor Green
