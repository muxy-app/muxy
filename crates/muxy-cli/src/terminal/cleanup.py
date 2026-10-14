import errno
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
report = b"\x1b[<0;5;5M\x1b[<0;5;5m"
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
    elif scenario == "input":
        os.write(master, report)
    os.kill(pid, signal.SIGCONT)
    if scenario == "suspend":
        os.write(master, report * 80)
    deadline = time.monotonic() + 3
    sent_late = False
    stops = 0
    while time.monotonic() < deadline:
        if master is not None and select.select([master], [], [], 0.001)[0]:
            try:
                raw += os.read(master, 65536)
                if b"\x1b[6n" in raw:
                    os.write(master, b"\x1b[1;1R")
                    raw = raw.replace(b"\x1b[6n", b"")
            except OSError:
                pass
        if scenario == "late-input" and not sent_late and b"\x1b[?1006l" in raw:
            time.sleep(0.001)
            os.write(master, report)
            sent_late = True
        if scenario == "continuous-input":
            try:
                os.write(master, report)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
        if scenario == "suspend" and b"\x1b[?1006l" not in raw:
            os.write(master, report)
        found, status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
        if found:
            if os.WIFSTOPPED(status):
                assert scenario == "suspend" and stops < 2, (status, raw)
                if stops == 0:
                    assert termios.tcgetattr(master) == before
                else:
                    os.write(master, b"x")
                stops += 1
                os.kill(pid, signal.SIGCONT)
                continue
            exited = True
            assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0, (status, raw)
            break
        time.sleep(0.001)
    assert exited, f"Terminal cleanup remained alive after {scenario}: {stops} stops, {raw!r}"
    if scenario == "late-input":
        assert sent_late
    if scenario == "suspend":
        assert stops == 2
    if master is not None:
        assert termios.tcgetattr(master) == before
finally:
    if master is not None:
        os.close(master)
    if not exited:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
