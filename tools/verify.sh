#!/usr/bin/env bash
# 各章の完成コード（chNN/）と付録 C の解答（appc/）がビルドでき、テストが通ることを確かめる。
#   tools/verify.sh            すべて
#   tools/verify.sh 8 9        指定した章だけ
#   tools/verify.sh appc       付録 C の解答だけ
#   tools/verify.sh viewer     ROM ビューアーだけ
# 環境変数 SKIP_WASM=1 で WebAssembly のビルドを省略する。
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)

chapters=("$@")
if [ ${#chapters[@]} -eq 0 ]; then
  chapters=(3 4 5 6 7 8 9 10 11 12 appc viewer)
fi

for ch in "${chapters[@]}"; do
  if [[ "$ch" == app* || "$ch" == viewer ]]; then
    name=$ch
  else
    name=$(printf "ch%02d" "$ch")
  fi
  echo "=== $name ==="
  (
    cd "$ROOT/$name"
    cargo test --quiet
    if [[ "$ch" == viewer || ( "$ch" != app* && "$ch" -ge 9 ) ]] && [ "${SKIP_WASM:-0}" != 1 ]; then
      cargo build --quiet --release --lib --target wasm32-unknown-unknown
    fi
  )
done
echo "すべて確認しました"
