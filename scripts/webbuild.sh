#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
web="$root/web"
pkg="$web/pkg"
help="$root/docs/web/STANDALONE_HTML.txt"

confirm_install() {
  local answer
  read -r -p "$1 [y/N] " answer
  [[ "$answer" =~ ^[Yy]([Ee][Ss])?$ ]] || {
    printf 'Installation declined; no build was attempted.\n' >&2
    exit 1
  }
}

printf 'Checking Rust toolchain...\n'
if ! command -v rustup >/dev/null 2>&1; then
  command -v curl >/dev/null 2>&1 || {
    printf 'rustup and curl are missing. Install curl, then rerun.\n' >&2
    exit 1
  }
  confirm_install 'Rustup is missing. Install it from the official rustup.rs bootstrap script?'
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
    sh -s -- -y --no-modify-path
  export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
fi
command -v cargo >/dev/null 2>&1 || {
  printf 'cargo is unavailable after the Rustup check. Source ~/.cargo/env and rerun.\n' >&2
  exit 1
}

printf 'Checking wasm32-unknown-unknown target...\n'
if ! rustup target list --installed | grep -Fqx 'wasm32-unknown-unknown'; then
  confirm_install 'The wasm32-unknown-unknown target is missing. Install it with rustup?'
  rustup target add wasm32-unknown-unknown
fi

printf 'Checking wasm-pack...\n'
if ! command -v wasm-pack >/dev/null 2>&1; then
  confirm_install 'wasm-pack is missing. Build and install it with cargo install wasm-pack --locked?'
  cargo install wasm-pack --locked
fi

printf 'Building browser package...\n'
(
  cd -- "$web"
  wasm-pack build . --target web --release --out-dir pkg
)
cp -- "$web/runner.js" "$pkg/runner.js"

for name in regedited_web.js regedited_web_bg.wasm runner.js package.json; do
  [[ -f "$pkg/$name" ]] || {
    printf 'Build reported success but required artifact is missing: %s\n' "$pkg/$name" >&2
    exit 1
  }
done

printf 'Web build complete. Generated files:\n'
find "$pkg" -maxdepth 1 -type f -print | sort | sed 's/^/  /'
printf 'Copy the complete package directory into a web project: %s\n' "$pkg"
printf 'Import runner.js for CLI-shaped JavaScript methods, or regedited_web.js for low-level bindings.\n'
printf 'Standalone HTML help: %s\n' "$help"
