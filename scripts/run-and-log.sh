#!/bin/bash
set -e

echo "==> Building..."
swift build

echo "==> Starting Muxy..."
swift run Muxy &
MUXYPID=$!

echo "==> Waiting for debug Muxy process..."
DBG_PID=""
for i in $(seq 1 30); do
    DBG_PID=$(ps aux | grep "[.]build/.*/debug/Muxy" | awk '{print $2}' | head -1)
    if [ -n "$DBG_PID" ]; then
        break
    fi
    sleep 1
done

if [ -z "$DBG_PID" ]; then
    echo "Error: debug Muxy not found after 30s"
    kill $MUXYPID 2>/dev/null
    exit 1
fi

echo "==> Debug PID: $DBG_PID"
echo "==> SSH logs (Ctrl+C to stop):"
log stream --predicate 'subsystem == "app.muxy" AND category == "SSHConnection"' --level debug --process "$DBG_PID"

kill $MUXYPID 2>/dev/null
