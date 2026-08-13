#!/usr/bin/env bash
set -euo pipefail
# Start rakuyomi CLI/server on a Kindle-like device, detach to background, log to /tmp

BINARY=${1:-/mnt/us/koreader/rakuyomi/rakuyomi-cli}
PIDFILE=${2:-/tmp/rakuyomi.pid}
LOG=${3:-/tmp/rakuyomi.log}

if [ ! -f "$BINARY" ]; then
  echo "Binary not found: $BINARY" >&2
  exit 2
fi

if [ ! -x "$BINARY" ]; then
  chmod +x "$BINARY" || true
fi

if [ -f "$PIDFILE" ]; then
  pid=$(cat "$PIDFILE" 2>/dev/null || true)
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    echo "rakuyomi already running (PID=$pid)"
    exit 0
  else
    rm -f "$PIDFILE"
  fi
fi

nohup "$BINARY" >/dev/null 2>>"$LOG" &
child=$!
# give it a moment to start
sleep 0.5
if kill -0 "$child" 2>/dev/null; then
  echo "$child" > "$PIDFILE"
  echo "Started $BINARY (PID=$child), logging to $LOG"
  exit 0
else
  echo "Failed to start $BINARY; check $LOG" >&2
  exit 3
fi
