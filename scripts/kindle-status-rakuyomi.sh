#!/usr/bin/env bash
set -euo pipefail
# Show rakuyomi status and tail log

PIDFILE=${1:-/tmp/rakuyomi.pid}
LOG=${2:-/tmp/rakuyomi.log}

if [ -f "$PIDFILE" ]; then
  pid=$(cat "$PIDFILE" 2>/dev/null || true)
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    echo "rakuyomi running (PID=$pid)"
  else
    echo "pidfile exists but process not running"
  fi
else
  echo "rakuyomi not running (no pidfile)"
fi

if [ -f "$LOG" ]; then
  echo "--- last 50 lines of $LOG ---"
  tail -n 50 "$LOG"
else
  echo "No log file at $LOG"
fi
