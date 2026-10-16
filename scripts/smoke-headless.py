#!/usr/bin/env python3
"""Exercise a built CLI with an isolated profile and an owned foreground server."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


def smoke(binary):
    with tempfile.TemporaryDirectory(prefix="muxy-smoke-") as temporary:
        profile = Path(temporary)
        env = {**os.environ, "MUXY_DIR": temporary, "SHELL": "/bin/sh"}
        def command(*args):
            return subprocess.check_output([binary, *args], env=env, text=True, timeout=10)
        for flag in ["--help", "--version", "--build-info"]:
            command(flag)
        assert not list(profile.iterdir()), "informational command created profile data"
        identity = None
        for restart in [False, True]:
            with (profile / "smoke.log").open("ab") as log:
                server = subprocess.Popen([binary.with_name("muxy-server")], env=env, stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 10
                    while not (profile / "server.sock").exists() or not (profile / "sessions/catalog.json").exists():
                        if server.poll() is not None or time.monotonic() >= deadline:
                            raise RuntimeError("server did not become ready")
                        time.sleep(0.025)
                    listed = command("project", "list")
                    assert "Home" in listed
                    if not restart:
                        command("project", "add", temporary, "--name", "Native project")
                        identity = command("project", "list")
                    else:
                        assert command("project", "list") == identity, "catalog changed across restart"
                    for name in ["sessions/catalog.json", "server.log", "server.toml"]:
                        assert (profile / name).stat().st_mode & 0o077 == 0, f"{name} is not private"
                    json.loads(command("--build-info"))
                finally:
                    if server.poll() is None:
                        server.send_signal(signal.SIGTERM)
                    try:
                        status = server.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                        raise
                    if status:
                        raise RuntimeError(f"server exited with {status}")
            assert not (profile / "server.sock").exists(), "server left its socket behind"
    print("Native CLI startup, projects, private storage, shutdown and restart passed")


if __name__ == "__main__":
    smoke(Path(sys.argv[1]).resolve())
