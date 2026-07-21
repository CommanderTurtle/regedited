$ErrorActionPreference = 'Stop'

function Confirm-Install {
    param([string]$Question)
    $answer = Read-Host "$Question [y/N]"
    if ($answer -notmatch '^(?i:y|yes)$') {
        throw 'Installation declined; no build was attempted.'
    }
}

$root = Split-Path -Parent $PSScriptRoot
$web = Join-Path $root 'web'
$pkg = Join-Path $web 'pkg'
$help = Join-Path $root 'docs\web\STANDALONE_HTML.txt'

Write-Host 'Checking Rust toolchain...'
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw 'rustup is missing and winget is unavailable. Install Rustup from https://rustup.rs, then rerun.'
    }
    Confirm-Install 'Rustup is missing. Install official WinGet package Rustlang.Rustup?'
    winget install --id Rustlang.Rustup --exact --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -ne 0) { throw "WinGet Rustup installation failed with exit code $LASTEXITCODE." }
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo is unavailable after the Rustup check. Open a new shell or repair Rustup.'
}

Write-Host 'Checking wasm32-unknown-unknown target...'
$targets = rustup target list --installed
if ($LASTEXITCODE -ne 0) { throw 'Could not list installed Rust targets.' }
if ($targets -notcontains 'wasm32-unknown-unknown') {
    Confirm-Install 'The wasm32-unknown-unknown target is missing. Install it with rustup?'
    rustup target add wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw "Rust target installation failed with exit code $LASTEXITCODE." }
}

Write-Host 'Checking wasm-pack...'
if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
    Confirm-Install 'wasm-pack is missing and has no package in the configured WinGet sources. Build and install it with cargo install wasm-pack --locked?'
    cargo install wasm-pack --locked
    if ($LASTEXITCODE -ne 0) { throw "wasm-pack installation failed with exit code $LASTEXITCODE." }
}

Write-Host 'Building browser package...'
Push-Location $web
try {
    wasm-pack build . --target web --release --out-dir pkg
    if ($LASTEXITCODE -ne 0) { throw "wasm-pack build failed with exit code $LASTEXITCODE." }
} finally {
    Pop-Location
}

Copy-Item -LiteralPath (Join-Path $web 'runner.js') -Destination (Join-Path $pkg 'runner.js') -Force

$required = @('regedited_web.js', 'regedited_web_bg.wasm', 'runner.js', 'package.json')
foreach ($name in $required) {
    $path = Join-Path $pkg $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Build reported success but required artifact is missing: $path"
    }
}

Write-Host 'Web build complete. Generated files:'
Get-ChildItem -LiteralPath $pkg -File | Sort-Object Name | ForEach-Object { Write-Host "  $($_.FullName)" }
Write-Host "Copy the complete package directory into a web project: $pkg"
Write-Host 'Import runner.js for CLI-shaped JavaScript methods, or regedited_web.js for low-level bindings.'
Write-Host "Standalone HTML help: $help"
