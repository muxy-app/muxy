import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

scenario = sys.argv[2]
pid, master = pty.fork()
if pid == 0:
    os.kill(os.getpid(), signal.SIGSTOP)
    os.environ["MUXY_TEST_HOST_CLEANUP"] = scenario
    os.execv(sys.argv[1], [sys.argv[1], "--exact",
             "terminal::tests::cleanup_handles_hangup_and_pending_mouse_reports", "--nocapture"])
exited = False
try:
    os.waitpid(pid, os.WUNTRACED)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 100, 0, 0))
    before = termios.tcgetattr(master)
    os.kill(pid, signal.SIGCONT)
    deadline = time.monotonic() + 10
    raw = b""
    while time.monotonic() < deadline:
        if select.select([master], [], [], 0.02)[0]:
            raw += os.read(master, 65536)
            if b"\x1b[6n" in raw:
                os.write(master, b"\x1b[1;1R")
                raw = raw.replace(b"\x1b[6n", b"")
        found, status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
        if found:
            if not os.WIFSTOPPED(status):
                exited = True
                raise AssertionError(f"Child exited before cleanup: {status}, {raw!r}")
            break
    else:
        raise AssertionError(f"Child did not reach cleanup: {raw!r}")
    if scenario == "hangup":
        os.close(master)
        master = None
    else:
        os.write(master, b"\x1b[<0;5;5M\x1b[<0;5;5m")
    os.kill(pid, signal.SIGCONT)
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        if master is not None and select.select([master], [], [], 0.02)[0]:
            try:
                raw += os.read(master, 65536)
            except OSError:
                pass
        found, status = os.waitpid(pid, os.WNOHANG)
        if found:
            exited = True
            assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0, (status, raw)
            break
        time.sleep(0.02)
    assert exited, f"Terminal cleanup remained alive after {scenario}"
    if master is not None:
        assert termios.tcgetattr(master) == before
finally:
    if master is not None:
        os.close(master)
    if not exited:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
