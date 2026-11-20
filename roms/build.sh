#!/usr/bin/env bash
# デモ ROM（roms/*.nes）をソースから作り直す。
#   - CHR は src/make_chr.py で作る（Python 3 が必要）
#   - アセンブルには nesasm/ のアセンブラを使う（Rust が必要）
set -euo pipefail
cd "$(dirname "$0")"

cargo build --quiet --release --manifest-path ../nesasm/Cargo.toml
NESASM=../nesasm/target/release/nesasm

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
python3 src/make_chr.py "$TMP/tiles.chr"

build() {
  local name=$1
  shift
  "$NESASM" "src/$name.asm" "$name.nes" --chr "$TMP/tiles.chr" --pad FF "$@"
}

build hello
build game
