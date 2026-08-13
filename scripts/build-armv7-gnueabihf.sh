#!/usr/bin/env bash
set -euo pipefail

# Build script for armv7-unknown-linux-gnueabihf (glibc)
# Usage: ./scripts/build-armv7-gnueabihf.sh [--install-tools]
#
# This script:
#  - adds the rust target
#  - shows recommended RUSTFLAGS for an optimized release
#  - runs cargo build --release --target=armv7-unknown-linux-gnueabihf

TARGET=armv7-unknown-linux-gnueabihf

if [ "${1-}" = "--install-tools" ]; then
  echo "Installing required toolchain (Debian/Ubuntu)..."
  sudo apt-get update
  sudo apt-get install -y gcc-arm-linux-gnueabihf binutils-arm-linux-gnueabihf
fi

echo "Adding Rust target: $TARGET"
rustup target add "$TARGET"

cat <<'EOF'
Recommended environment variables for an optimized binary:
  export RUSTFLAGS='-C codegen-units=1 -C lto=fat -C opt-level=3 -C target-cpu=cortex-a8 -C link-arg=-s'
  (adjust target-cpu as appropriate for your Paperwhite SoC)

Recommended Cargo profile (add to workspace Cargo.toml under [profile.release]):
  opt-level = 3
  lto = "fat"
  codegen-units = 1
  panic = "abort"

Notes:
- If the device's libc is not compatible, consider musl (armv7-unknown-linux-musleabihf) or building statically.
- Use arm-linux-gnueabihf-strip on the produced binary to reduce size further.
EOF

export RUSTFLAGS="${RUSTFLAGS-'-C codegen-units=1 -C lto=fat -C opt-level=3 -C target-cpu=cortex-a8 -C link-arg=-s'}"

echo "Building release for target $TARGET..."
cargo build --release --target "$TARGET"

echo "Build artifacts are in: target/$TARGET/release/"

echo "Example: strip the binary to reduce size (replace 'server' with your binary name):"
echo "  arm-linux-gnueabihf-strip target/$TARGET/release/server || true"

echo "If you need further size reduction, consider using upx (may not be appropriate for all binaries):"
echo "  upx --lzma target/$TARGET/release/server"
