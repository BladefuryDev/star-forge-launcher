#!/usr/bin/env bash
# Builds everything into dist/<platform>/:
#   sc-inject.exe + sc_agent.dll  (always Windows, cross-built with cargo-xwin)
#   sc-launch                     (native Linux) and sc-launch.exe (Windows)
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
[ -f .env ] && { set -a; . ./.env; set +a; }
export PATH="$HOME/.cargo/bin:$PATH"

WIN=x86_64-pc-windows-msvc
cargo xwin build --release --target "$WIN"
cargo build --release -p sc-launch

# Copy to a temp name then rename, so a copy that is running (or loaded into the
# game) keeps its old file instead of being overwritten underneath it.
put() {
  mkdir -p "$2"
  cp "$1" "$2/.$(basename "$1").new"
  mv -f "$2/.$(basename "$1").new" "$2/$(basename "$1")"
}
for platform in linux windows; do
  put "target/$WIN/release/sc-inject.exe" "dist/$platform"
  put "target/$WIN/release/sc_agent.dll" "dist/$platform"
done
put target/release/sc-launch dist/linux
put "target/$WIN/release/sc-launch.exe" dist/windows
echo "built into dist/linux and dist/windows"
