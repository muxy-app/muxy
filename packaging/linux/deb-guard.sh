#!/bin/sh
set -eu

case "${1:-}" in
    install|upgrade|remove|deconfigure) ;;
    *) exit 0 ;;
esac

for process in /proc/[0-9]*/exe; do
    executable=$(readlink "$process" 2>/dev/null) || continue
    case "$executable" in
        /usr/lib/@PACKAGE@/*)
            echo 'Quit @PACKAGE@ and stop its server before upgrading or removing it.' >&2
            echo 'Stopping the server ends its running terminals; no processes were stopped automatically.' >&2
            exit 1
            ;;
    esac
done
