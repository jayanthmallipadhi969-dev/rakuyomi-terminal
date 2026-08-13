Cross-compilation guide: armv7-unknown-linux-gnueabihf

Purpose

This document describes how to build the Rust binaries in this repository for a Kindle Paperwhite (armv7l) running a glibc-based userland.

Overview

Targets considered:
- armv7-unknown-linux-gnueabihf (Recommended) — dynamically linked against glibc on the device. Smaller runtime footprint if device libc is compatible.
- armv7-unknown-linux-musleabihf (Alternative) — static musl-linked binary, easier deployment but usually larger.

Repository helpers

- .cargo/config.toml — sets the default target and the C linker/ar to use for armv7-unknown-linux-gnueabihf
- scripts/build-armv7-gnueabihf.sh — helper script that adds the rust target, recommends RUSTFLAGS and profile settings, and builds the workspace for the target

Prerequisites (build host)

1. Rust toolchain (rustup recommended)
   - Install rustup: https://rustup.rs
   - Ensure cargo/rustc/rustup are on PATH

2. Cross C toolchain (Debian/Ubuntu example)
   - sudo apt-get update
   - sudo apt-get install -y gcc-arm-linux-gnueabihf binutils-arm-linux-gnueabihf
   - These provide arm-linux-gnueabihf-gcc and arm-linux-gnueabihf-ar which the build expects

3. (Optional) upx for extra binary compression
   - sudo apt-get install -y upx-ucl

Recommended Cargo settings

Add or confirm the following in the workspace Cargo.toml (the backend workspace already contains these):

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"

Recommended environment variables for build

export RUSTFLAGS='-C codegen-units=1 -C lto=fat -C opt-level=3 -C target-cpu=cortex-a8 -C link-arg=-s'
Adjust "target-cpu" to match the Paperwhite SoC (cortex-a8 is a conservative common armv7 target).

Build steps (example)

1. On the build host, ensure rustup is present and add the target:
   rustup target add armv7-unknown-linux-gnueabihf

2. Install the cross C toolchain (Debian/Ubuntu):
   sudo apt-get install -y gcc-arm-linux-gnueabihf binutils-arm-linux-gnueabihf

3. Run the helper script from the repo root (or use the Makefile):
   ./scripts/build-armv7-gnueabihf.sh

   Or use the Makefile targets:
     make cross-build      # build glibc-linked armv7 release (uses rustup target)
     make musl-build       # build static musl release (requires `cross`)
     make strip            # strip the gnueabihf release binary

4. Strip the produced binary to reduce size (replace server/rakuyomi-cli as needed):
   arm-linux-gnueabihf-strip target/armv7-unknown-linux-gnueabihf/release/rakuyomi-cli || true

5. Deploy to your Kindle via scp and run (helper script and Makefile targets):
   ./scripts/deploy-to-kindle.sh target/armv7-unknown-linux-gnueabihf/release/rakuyomi-cli 192.168.1.42 /mnt/us/koreader/rakuyomi root
   or
   make install KINDLE_HOST=192.168.1.42 KINDLE_USER=root KINDLE_PATH=/mnt/us/koreader/rakuyomi
   make run KINDLE_HOST=192.168.1.42

6. (Optional) Compress with upx:
   upx --lzma target/armv7-unknown-linux-gnueabihf/release/rakuyomi-cli

Troubleshooting

- rustup: command not found
  - Install rustup from https://rustup.rs and reopen the shell. The build environment in some CI/containers may not include rustup by default.

- linker not found: arm-linux-gnueabihf-gcc
  - Install the cross toolchain package on your distro (example above). Alternatively, point .cargo/config.toml to a different linker you provide.

- incompatible libc on device (binary runs but fails to start)
  - Check the device libc version: run `ldd --version` or inspect /lib/ld-*.so on the device if you can. If incompatible, build musl static target instead (armv7-unknown-linux-musleabihf).

- Too-large binary
  - Use LTO, strip, set panic = "abort", reduce dependencies, enable features conditionally. For absolute smallest size, consider rewriting hot paths in C or using smaller runtime crates.

On-device steps

- Copy binary to device (example via scp or mounting):
  scp target/armv7-unknown-linux-gnueabihf/release/rakuyomi-cli kindle:/path/to/destination

- On-device permissions:
  chmod +x /path/to/destination/rakuyomi-cli

- Run and test:
  /path/to/destination/rakuyomi-cli --help

On-device service scripts (for Kindle terminal)

This repository includes simple start/stop/status scripts you can copy to the device to manage rakuyomi from the Kindle shell. They use a PID file (/tmp/rakuyomi.pid) and log to /tmp/rakuyomi.log.

Files added:
- scripts/kindle-start-rakuyomi.sh — Start the binary in the background: creates pidfile and appends logs to /tmp/rakuyomi.log
- scripts/kindle-stop-rakuyomi.sh — Stop the running process using the pidfile
- scripts/kindle-status-rakuyomi.sh — Check PID and tail the log

Example on-device usage (after copying the binary and scripts to the device):

# make scripts executable (on device)
chmod +x /path/to/kindle-start-rakuyomi.sh /path/to/kindle-stop-rakuyomi.sh /path/to/kindle-status-rakuyomi.sh

# start the daemon (uses default path /mnt/us/koreader/rakuyomi/rakuyomi-cli)
/path/to/kindle-start-rakuyomi.sh /mnt/us/koreader/rakuyomi/rakuyomi-cli

# check status and view recent logs
/path/to/kindle-status-rakuyomi.sh

# stop the daemon
/path/to/kindle-stop-rakuyomi.sh

Musl (static) build — recommended for portability

If device libc compatibility is uncertain, use the musl static build. A GitHub Actions workflow has been added to produce a musl static artifact automatically: .github/workflows/build-armv7-musleabihf.yml. The artifact name is armv7-musleabihf-release and will contain statically linked binaries that are more likely to run on varied armv7 devices without requiring matching glibc on the device.

Notes

If device libc compatibility is uncertain, use the musl static build. A GitHub Actions workflow has been added to produce a musl static artifact automatically: .github/workflows/build-armv7-musleabihf.yml. The artifact name is armv7-musleabihf-release and will contain statically linked binaries that are more likely to run on varied armv7 devices without requiring matching glibc on the device.

Notes

- This environment attempted to run `rustup target add` but rustup was not available here. The helper script and .cargo/config.toml have been added to the repo so they work on a developer machine or CI with rustup installed.

Contact

If preferred, provide remote access to a build host with rustup and cross toolchain and this repository can be built and a sample binary produced and checked-in as an artifact.
