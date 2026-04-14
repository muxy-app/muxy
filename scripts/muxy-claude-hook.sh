#!/usr/bin/env bash
set -euo pipefail

if [ -z "${MUXY_SOCKET_PATH:-}" ] || [ -z "${MUXY_PANE_ID:-}" ]; then
    exit 0
fi

event="${1:-}"
input=$(cat)

send_notification() {
    local type="$1"
    local title="$2"
    local body="$3"
    local json
    json=$(printf '{"type":"%s","paneID":"%s","title":"%s","body":"%s"}' \
        "$type" "$MUXY_PANE_ID" "$title" "$body")
    printf '%s' "$json" | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

extract_transcript_summary() {
    python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    tp = data.get('transcript_path', '')
    if tp:
        import pathlib
        lines = pathlib.Path(tp).read_text().strip().split('\n')
        for line in reversed(lines):
            entry = json.loads(line)
            msg = entry.get('message', {})
            if msg.get('role') == 'assistant':
                content = msg.get('content', '')
                if isinstance(content, list):
                    for block in content:
                        if isinstance(block, dict) and block.get('type') == 'text':
                            content = block.get('text', '')
                            break
                text = str(content).replace('\"', '').replace('\\\\', '')[:200]
                print(text)
                sys.exit(0)
    cwd = data.get('cwd', '')
    if cwd:
        import os
        print('Completed in ' + os.path.basename(cwd))
    else:
        print('Session completed')
except Exception:
    print('Session completed')
" 2>/dev/null <<< "$input" || echo "Session completed"
}

case "$event" in
    notification)
        body=$(python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    print(data.get('message', 'Needs attention'))
except Exception:
    print('Needs attention')
" 2>/dev/null <<< "$input" || echo "Needs attention")
        send_notification "claude_hook" "Claude Code" "$body"
        ;;
    stop)
        body=$(extract_transcript_summary)
        send_notification "claude_hook" "Claude Code" "$body"
        ;;
esac
