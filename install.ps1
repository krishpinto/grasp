# Grasp one-line installer (downloads the prebuilt bundle — model included).
#
#   irm https://github.com/krishpinto/grasp/releases/latest/download/install.ps1 | iex
#
# No Rust, no build, no separate model download.

$ErrorActionPreference = "Stop"
$repo = "krishpinto/grasp"
$dest = Join-Path $env:LOCALAPPDATA "Grasp"
$zip = Join-Path $env:TEMP "grasp-windows-x64.zip"
$url = "https://github.com/$repo/releases/latest/download/grasp-windows-x64.zip"

Write-Host "Downloading Grasp (this includes the embedding model)..." -ForegroundColor Cyan
Invoke-WebRequest $url -OutFile $zip

Write-Host "Installing to $dest..."
if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Expand-Archive -Path $zip -DestinationPath $dest -Force
Remove-Item $zip -Force

$exe = Join-Path $dest "grasp.exe"
if (-not (Test-Path $exe)) { Write-Error "Install failed: $exe not found."; exit 1 }

Write-Host "Registering with Claude Code..."
& $exe setup

Write-Host "Importing your existing sessions..."
& $exe import

Write-Host "Enabling always-on background capture..."
& $exe autostart

Write-Host ""
Write-Host "Done! Grasp is installed at $dest" -ForegroundColor Green
Write-Host "Open a Claude Code session and ask it about your past work."
Write-Host "Background capture starts at next login (run '$dest\grasp.exe watch' to start it now)."
Write-Host "Tip: add '$dest' to your PATH to run 'grasp' from anywhere."
