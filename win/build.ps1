# Compile the standalone Windows 64-bit MindMap program into this folder.
# Install Rust from https://rustup.rs first, then run from the repository:
#   .\win\build.ps1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Force -Path (Join-Path $Root "win") | Out-Null
Copy-Item -Force (Join-Path $Root "target\release\mindmap.exe") (Join-Path $Root "win\mindmap.exe")
Write-Host "Built win\mindmap.exe"
Write-Host "Canvases are saved in win\mindmap-data\ beside the program."
