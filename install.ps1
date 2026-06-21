# Engram installer (Windows, from source).
# Builds the engine, registers it with Claude Code, and imports your history.
#
#   powershell -ExecutionPolicy Bypass -File .\install.ps1

$ErrorActionPreference = "Stop"

Write-Host "🧠 Installing Engram..." -ForegroundColor Cyan

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Rust/cargo not found. Install it from https://rustup.rs first, then re-run."
    exit 1
}

Write-Host "Building the engine (release) - first build can take a few minutes..."
cargo build -p engram-cli --release

$exe = Join-Path (Get-Location) "target\release\engram.exe"
if (-not (Test-Path $exe)) {
    Write-Error "Build finished but $exe is missing."
    exit 1
}

Write-Host "Registering with Claude Code..."
& $exe setup

Write-Host "Importing your existing sessions..."
& $exe import

Write-Host ""
Write-Host "Done." -ForegroundColor Green
Write-Host "Next steps:"
Write-Host "  - Enable semantic search (one-time, ~90MB):  $exe embed"
Write-Host "  - Open the desktop app:                      pnpm install; pnpm tauri dev"
Write-Host "  - Or just open Claude Code and ask it about your past work."
