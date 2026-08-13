#!/usr/bin/env bash
set -euo pipefail
# Stop rakuyomi process started by kindle-start-rakuyomi.sh

PIDFILE=${1:-/tmp/rakuyomi.pid}

if [ ! -f "$PIDFILE" ]; then
  echo "No pidfile at $PIDFILE; rakuyomi not running?"
  exit 0
fi

pid=$(cat "$PIDFILE")
if [ -z "$pid" ]; then
  echo "Pidfile empty; removing"
  rm -f "$PIDFILE"
  exit 0
fi

if kill "$pid" 2>/dev/null; then
  echo "Stopped rakuyomi (PID=$pid)"
  rm -f "$PIDFILE"
  exit 0
else
  echo "Failed to stop PID $pid; trying kill -9"
  kill -9 "$pid" 2>/dev/null || true
  rm -f "$PIDFILE"
  exit 0
fi
