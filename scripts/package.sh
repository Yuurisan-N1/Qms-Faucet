#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NAME="QMSFaucet-Miniapp"
STAMP="$(date +%Y%m%d)"
STAGE="/tmp/$NAME-$STAMP"
OUT="/root/$NAME.zip"

rm -rf "$STAGE"
mkdir -p "$STAGE"

copy() { [ -e "$ROOT/$1" ] && cp -r "$ROOT/$1" "$STAGE/$1" || true; }

copy src
copy assets
copy scripts
copy .github
copy Cargo.toml
copy Cargo.lock
copy build.rs
copy Makefile
copy README.md
copy LICENSE
copy .gitignore
copy config.json
copy run.sh

mkdir -p "$STAGE/cuda/include" "$STAGE/cuda/kernels" "$STAGE/cuda/arch" "$STAGE/cuda/tests"
cp "$ROOT"/cuda/include/*.h   "$STAGE/cuda/include/" 2>/dev/null || true
cp "$ROOT"/cuda/kernels/*.cu  "$ROOT"/cuda/kernels/*.cuh "$STAGE/cuda/kernels/" 2>/dev/null || true
cp "$ROOT"/cuda/arch/*.cuh    "$STAGE/cuda/arch/" 2>/dev/null || true
cp "$ROOT"/cuda/tests/*       "$STAGE/cuda/tests/" 2>/dev/null || true

: > "$STAGE/address.txt"
: > "$STAGE/proxy.txt"

chmod +x "$STAGE/scripts/"*.sh 2>/dev/null || true

rm -f "$OUT"
( cd "$STAGE" && zip -rq9 "$OUT" . )
rm -rf "$STAGE"

echo "package : $OUT"
echo "files   : $(unzip -l "$OUT" | tail -n1 | awk '{print $2}')"
sha256sum "$OUT"
