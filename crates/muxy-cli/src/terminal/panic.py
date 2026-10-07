import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

pid, master = pty.fork()
if pid == 0:
    os.kill(os.getpid(), signal.SIGSTOP)
    os.environ["MUXY_TEST_HOST_PANIC"] = "1"
    os.execv(sys.argv[1], [sys.argv[1], "--exact",
             "terminal::tests::panic_restores_terminal_settings_and_output_modes", "--nocapture"])
exited = False
try:
    os.waitpid(pid, os.WUNTRACED)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 100, 0, 0))
    before = subprocess.check_output(["stty", "-g"], stdin=master)
    os.kill(pid, signal.SIGCONT)
    deadline = time.monotonic() + 10
    raw = b""
    while time.monotonic() < deadline:
        if select.select([master], [], [], 0.02)[0]:
            try:
                data = os.read(master, 65536)
                raw += data
                if b"\x1b[6n" in data:
                    os.write(master, b"\x1b[1;1R")
            except OSError:
                pass
        found, status = os.waitpid(pid, os.WNOHANG)
        if found:
            exited = True
            assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 101, status
            break
    assert exited, "panic child did not exit"
    assert subprocess.check_output(["stty", "-g"], stdin=master) == before
    for sequence in [b"\x1b[?1049h", b"\x1b[?1049l", b"\x1b[?2004l", b"\x1b[?1004l", b"\x1b[?25h",
                     b"\x1b[?1000l", b"\x1b[?1003l", b"\x1b[?1006l"]:
        assert sequence in raw, (sequence, raw)
finally:
    if not exited:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    os.close(master)
