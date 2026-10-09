#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/cuda/kernels/argon2_kernel.cu"
CC="compute_"

pick_nvcc() {
    if [ -n "${NVCC:-}" ]; then echo "$NVCC"; return; fi
    if command -v nvcc >/dev/null 2>&1; then command -v nvcc; return; fi
    if command -v nvcc.exe >/dev/null 2>&1; then command -v nvcc.exe; return; fi
    for d in /usr/local/cuda /usr/local/cuda-* /opt/cuda /opt/cuda-*; do
        [ -x "$d/bin/nvcc" ] && { echo "$d/bin/nvcc"; return; }
    done
    for d in "/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA"/* \
             "/mnt/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA"/*; do
        [ -x "$d/bin/nvcc.exe" ] && { echo "$d/bin/nvcc.exe"; return; }
    done
    echo ""
}

cap() {
    if [ -n "${QMS_CUDA_ARCH:-}" ]; then echo "$QMS_CUDA_ARCH"; return; fi
    if command -v nvidia-smi >/dev/null 2>&1; then
        local c
        c="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader 2>/dev/null \
             | head -n1 | tr -d ' .')"
        if [ -n "$c" ]; then echo "$c"; return; fi
    fi
    echo "75"
}
NVCC="$(pick_nvcc)"
if [ -z "$NVCC" ]; then
    echo "nvcc was not found, so the accelerator cannot be built."
    echo "Install the CUDA toolkit, or just skip it: the bot runs on the cpu."
    exit 1
fi

NV_MAJOR="$("$NVCC" --version | sed -n 's/.*release \([0-9][0-9]*\).*/\1/p' | head -n1)"
NV_MAJOR="${NV_MAJOR:-0}"
CAP="$(cap)"

CAP_DETECTED=0
if [ -n "${QMS_CUDA_ARCH:-}" ]; then
    CAP_DETECTED=1
elif command -v nvidia-smi >/dev/null 2>&1; then
    if [ -n "$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader 2>/dev/null \
                | head -n1 | tr -d ' .')" ]; then
        CAP_DETECTED=1
    fi
fi

SUPPORTED=""
if "$NVCC" --list-gpu-arch >/dev/null 2>&1; then
    SUPPORTED="$("$NVCC" --list-gpu-arch 2>/dev/null \
                 | sed -n 's/^compute_\([0-9][0-9]*\)$/\1/p' | tr '\n' ' ')"
fi

arch_ok() {
    if [ -n "$SUPPORTED" ] && ! printf ' %s ' "$SUPPORTED" | grep -q " $1 "; then
        return 1
    fi
    if [ "$1" -lt 75 ] && [ "$NV_MAJOR" -ge 13 ]; then
        return 1
    fi
    return 0
}

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) OS="windows" ;;
    *) OS="unix" ;;
esac

CUDA_HOME="$(cd "$(dirname "$NVCC")/.." && pwd)"
if [ "$OS" = "windows" ]; then
    LIB="qms_cuda.dll"
    LIBDIR="$CUDA_HOME/lib/x64"
else
    LIB="libqms_cuda.so"
    LIBDIR="$CUDA_HOME/lib64"
    [ -d "$LIBDIR" ] || LIBDIR="$CUDA_HOME/lib"
fi

OUT="$ROOT/cuda/$LIB"

if ! arch_ok "$CAP"; then
    echo "cuda $NV_MAJOR cannot target sm_$CAP, so the accelerator is skipped."
    exit 1
fi

GEN=""
FAT_ARCHS=""
for a in $("$NVCC" --list-gpu-arch 2>/dev/null | sed -n 's/^compute_\([0-9][0-9]*\)$/\1/p'); do
    arch_ok "$a" || continue
    FAT_ARCHS="$FAT_ARCHS $a"
done
[ -n "$FAT_ARCHS" ] || FAT_ARCHS=" 60 61 70 75 86 89 90"

if [ -z "${QMS_CUDA_FAST:-}" ]; then
    for a in $FAT_ARCHS; do
        GEN="$GEN -gencode arch=${CC}$a,code=sm_$a"
    done
fi
GEN="$GEN -gencode arch=${CC}$CAP,code=sm_$CAP"

PTX_ARCH="$CAP"
for a in $FAT_ARCHS 120 100 121; do
    if [ "$a" -ge "$CAP" ] && arch_ok "$a"; then PTX_ARCH="$a"; fi
done
GEN="$GEN -gencode arch=${CC}$PTX_ARCH,code=compute_$PTX_ARCH"

if [ -f "$LIBDIR/libcudart_static.a" ] || [ -f "$LIBDIR/cudart_static.lib" ]; then
    LINK="-lcudart_static"
    [ "$OS" = "unix" ] && LINK="$LINK -lpthread -ldl -lrt"
else
    LINK="-lcudart"
fi

echo "nvcc      : $NVCC (cuda $NV_MAJOR)"
echo "arch      : $CAP"
echo "output    : $OUT"
echo "flags     :$GEN"
echo "compiling : let it finish, this takes under a minute"

build() {
    "$NVCC" -O3 -shared -std=c++17 \
        -Xcompiler -fPIC \
        -Wno-deprecated-gpu-targets \
        -I"$ROOT/cuda/include" -I"$ROOT/cuda/kernels" \
        -L"$LIBDIR" \
        $1 \
        -o "$OUT" "$SRC" $LINK
}
if ! build "$GEN"; then
    echo "the fat build failed, probing the archs one by one"
    GEN=""
    for a in $FAT_ARCHS; do
        if build "-gencode arch=${CC}$a,code=sm_$a"; then
            GEN="$GEN -gencode arch=${CC}$a,code=sm_$a"
        else
            echo "  sm_$a cannot be built by this toolkit, skipped"
        fi
    done
    GEN="$GEN -gencode arch=${CC}$CAP,code=sm_$CAP"
    GEN="$GEN -gencode arch=${CC}$PTX_ARCH,code=compute_$PTX_ARCH"
    if ! build "$GEN"; then
        echo "the cuda accelerator could not be built, the bot will use the cpu."
        rm -f "$OUT"
        exit 1
    fi
fi

if command -v cuobjdump >/dev/null 2>&1; then
    CU_LIST="$(cuobjdump --list-elf "$OUT" 2>/dev/null || true)"
    case "$CU_LIST" in
        *.cubin*) : ;;
        *)
            echo "the accelerator has no kernel image for any arch, discarding it."
            rm -f "$OUT"
            exit 1
            ;;
    esac
fi

if command -v nm >/dev/null 2>&1; then
    echo "exported  : $(nm -D --defined-only "$OUT" 2>/dev/null | grep -c qms_ || echo '?') qms symbols"
fi

echo "done. the bot picks this up on the next run."
