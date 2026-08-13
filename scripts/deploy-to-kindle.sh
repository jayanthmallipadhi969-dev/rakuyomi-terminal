#!/usr/bin/env bash
# Deploy a built rakuyomi binary to a Kindle and optionally run it.
# Usage: ./scripts/deploy-to-kindle.sh <binary-path> <kindle-host> [kindle-path] [ssh-user]
# Example: ./scripts/deploy-to-kindle.sh target/armv7-unknown-linux-gnueabihf/release/rakuyomi-cli 192.168.1.42 /mnt/us/koreader/rakuyomi root

set -euo pipefail

BINARY=${1:-}
KINDLE_HOST=${2:-}
KINDLE_PATH=${3:-/mnt/us/koreader/rakuyomi}
KINDLE_USER=${4:-root}

if [ -z "$BINARY" ] || [ -z "$KINDLE_HOST" ]; then
  echo "Usage: $0 <binary-path> <kindle-host> [kindle-path] [ssh-user]"
  exit 2
fi

if [ ! -f "$BINARY" ]; then
  echo "Binary not found: $BINARY"
  exit 1
fi

echo "Copying $BINARY to $KINDLE_USER@$KINDLE_HOST:$KINDLE_PATH"
scp "$BINARY" "$KINDLE_USER@$KINDLE_HOST:$KINDLE_PATH/"

echo "Setting executable and running on device (shows --help output)"
ssh "$KINDLE_USER@$KINDLE_HOST" "chmod +x $KINDLE_PATH/$(basename "$BINARY") && $KINDLE_PATH/$(basename "$BINARY") --help"

echo "Deployed and ran $BINARY on $KINDLE_HOST"
