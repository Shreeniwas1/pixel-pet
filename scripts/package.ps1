# Builds a release (tests must pass) and zips it for distribution.
# Usage: powershell -File scripts\package.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
& "$PSScriptRoot\release.ps1"
if ($LASTEXITCODE -ne 0) { exit 1 }

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
$stage = Join-Path $root "dist\PixelPet-$version"
Remove-Item (Join-Path $root "dist") -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item target\release\pixel-pet.exe $stage
Copy-Item assets\README.txt $stage
Copy-Item NOTICE.md (Join-Path $stage "NOTICE.txt")
Copy-Item LICENSE (Join-Path $stage "LICENSE.txt")
Copy-Item LICENSE-ART.md (Join-Path $stage "LICENSE-ART.txt")
$zip = Join-Path $root "dist\PixelPet-$version.zip"
Compress-Archive -Path "$stage\*" -DestinationPath $zip -Force
$h = (Get-FileHash $zip -Algorithm SHA256).Hash
Write-Host "OK: $zip" -ForegroundColor Green
Write-Host "SHA256: $h"
