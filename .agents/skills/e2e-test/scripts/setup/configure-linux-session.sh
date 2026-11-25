#!/bin/bash
# Root, in the guest. Reboot after switching session type.
set -euo pipefail
session_type="${1:?Usage: configure-linux-session.sh x11|wayland}"
case "$session_type" in x11|wayland) ;; *) exit 2 ;; esac
id muxy >/dev/null
python3 - "$session_type" <<'PY'
import configparser
from pathlib import Path
import sys
kind=sys.argv[1]
session='ubuntu-xorg' if kind == 'x11' else 'ubuntu'
p=Path('/etc/gdm3/custom.conf')
config=configparser.ConfigParser(strict=False)
config.optionxform=str
config.read(p)
if not config.has_section('daemon'):
    config.add_section('daemon')
config['daemon'].update({
    'WaylandEnable': 'false' if kind == 'x11' else 'true',
    'AutomaticLoginEnable': 'true',
    'AutomaticLogin': 'muxy',
    'DefaultSession': session + '.desktop',
})
with p.open('w') as out:
    config.write(out, space_around_delimiters=False)
p=Path('/var/lib/AccountsService/users/muxy')
p.parent.mkdir(parents=True, exist_ok=True)
p.write_text('[User]\nSession=' + session + '\nXSession=' + session + '\nSystemAccount=false\n')
p.chmod(0o600)
PY
