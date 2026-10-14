#!/usr/bin/env python3
"""Exercise desktop/TUI coexistence against an isolated local runtime."""

import os
from pathlib import Path
import shlex
import signal
import subprocess
import tempfile


def main():
    root = Path(__file__).resolve().parent.parent
    binary = root / "target/debug/muxy"
    server = binary.with_name("muxy-server")
    with tempfile.TemporaryDirectory(prefix="muxy-tui-desktop-", dir="/tmp") as temporary:
        profile = Path(temporary)
        home = profile / "home"
        home.mkdir()
        (home / ".zshrc").write_text("PS1='tui-desktop> '\nRPS1=''\n")
        (profile / "server.toml").write_text('default_shell = "/bin/zsh"\n')
        wrapper = profile / "server-wrapper.sh"
        wrapper.write_text(
            '#!/bin/sh\nprintf "%s\\n" "$$" > "$MUXY_DIR/server.pid"\n'
            f'exec {shlex.quote(str(server))} "$@"\n'
        )
        wrapper.chmod(0o700)
        env = {
            **os.environ,
            "CARGO_HOME": os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")),
            "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup")),
            "MUXY_DIR": str(profile),
            "HOME": str(home),
            "ZDOTDIR": str(home),
            "SHELL": "/bin/zsh",
            "MUXY_TEST_TUI": str(binary),
            "MUXY_SERVER_BIN": str(wrapper),
        }
        build = subprocess.run(
            ["cargo", "test", "--locked", "-p", "muxy-app", "--no-run"],
            cwd=root, env=env, check=False,
        ).returncode
        if build:
            return build
        try:
            return subprocess.run(
                ["cargo", "test", "--locked", "-p", "muxy-app",
                 "nested_tui_and_desktop_share_a_session_with_independent_layouts",
                 "--", "--ignored", "--nocapture"],
                cwd=root, env=env, timeout=180, check=False,
            ).returncode
        finally:
            record = profile / "server.pid"
            if record.exists():
                pid = int(record.read_text())
                command = subprocess.run(
                    ["ps", "-p", str(pid), "-o", "command="],
                    capture_output=True, text=True, check=False,
                ).stdout
                if str(server) in command and str(profile) in command:
                    try:
                        os.kill(pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass


if __name__ == "__main__":
    raise SystemExit(main())
