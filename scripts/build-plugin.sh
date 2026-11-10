#!/usr/bin/env bash
# Build and package the pawpeek plugin.
set -euo pipefail

echo "=== Building PawPeek Plugin ==="

PLATFORM="${1:-}"
BINARY_PATH="${2:-}"

if [[ -z "$PLATFORM" ]]; then
    if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
        PLATFORM="windows"; BINARY_PATH="target/release/pawpeek.exe"
    elif [[ "$(uname -s)" == "Darwin" ]]; then
        PLATFORM="macos"; BINARY_PATH="target/release/pawpeek"
    else
        PLATFORM="linux"; BINARY_PATH="target/release/pawpeek"
    fi
    echo "Building for ${PLATFORM}..."
    cargo build --release
fi

if [[ -z "$BINARY_PATH" ]]; then
    if [[ "$PLATFORM" == "windows" ]]; then
        BINARY_PATH="target/release/pawpeek.exe"
    else
        BINARY_PATH="target/release/pawpeek"
    fi
fi

if [[ ! -f "$BINARY_PATH" ]]; then
    echo "Error: Binary not found: ${BINARY_PATH}" >&2
    exit 1
fi

PACKAGE="dist/pawpeek-plugin-${PLATFORM}.zip"
echo "Platform: ${PLATFORM}"
echo "Packaging to ${PACKAGE}..."

mkdir -p dist

# 打包資料夾保留在 dist/ 下，可直接當本機 marketplace：
#   claude plugin marketplace add <repo>/dist/pawpeek-plugin-<platform>
TMPDIR="$(pwd)/dist/pawpeek-plugin-${PLATFORM}"
rm -rf "${TMPDIR}"
mkdir -p "${TMPDIR}"

cp -r .claude-plugin "${TMPDIR}/"
cp .claude-plugin/marketplace.json "${TMPDIR}/"
cp -r hooks "${TMPDIR}/"
cp -r commands "${TMPDIR}/"
cp -r assets "${TMPDIR}/"
cp PLUGIN.md "${TMPDIR}/" 2>/dev/null || true
cp README.md "${TMPDIR}/" 2>/dev/null || true

mkdir -p "${TMPDIR}/bin"
cp "${BINARY_PATH}" "${TMPDIR}/bin/"

# GNU toolchain 編出的 Windows exe 需要旁邊有 WebView2Loader.dll（MSVC 版不需要）
if [[ "$PLATFORM" == "windows" ]]; then
    DLL="$(dirname "${BINARY_PATH}")/WebView2Loader.dll"
    [[ -f "$DLL" ]] || DLL=$(find target/release/build -path "*x64*" -name WebView2Loader.dll 2>/dev/null | head -1)
    if [[ -n "$DLL" && -f "$DLL" ]]; then cp "$DLL" "${TMPDIR}/bin/"; fi
fi

cd "${TMPDIR}"
zip -r "${OLDPWD}/${PACKAGE}" .
cd "${OLDPWD}"

SIZE=$(du -h "${PACKAGE}" | cut -f1)
echo ""
echo "=== Package created ==="
echo "Folder: ${TMPDIR}"
echo "File: ${PACKAGE}"
echo "Size: ${SIZE}"
