import contextlib
import io
import os
from pathlib import Path
import signal
import subprocess
import sys
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import slow_test_guard as GUARD  # noqa: E402


class SlowTestGuardTests(unittest.TestCase):
    def guarded(self, script):
        output = io.TextIOWrapper(io.BytesIO())
        try:
            with contextlib.redirect_stdout(output):
                GUARD.run([sys.executable, "-c", textwrap.dedent(script)])
        finally:
            self.output = output.buffer.getvalue().decode()

    def test_a_test_the_harness_reports_as_slow_stops_the_run(self):
        with self.assertRaisesRegex(TimeoutError, "test fake::hangs ran for over 60 seconds"):
            self.guarded("""
                import time
                print("running 2 tests")
                print("test fake::hangs has been running for over", 60, "seconds", flush=True)
                time.sleep(60)
            """)

    def test_a_failed_command_fails_like_a_checked_subprocess_run(self):
        with self.assertRaises(subprocess.CalledProcessError) as failure:
            self.guarded("import sys; sys.exit(3)")
        self.assertEqual(failure.exception.returncode, 3)

    def test_processes_left_behind_by_a_finished_command_do_not_hold_the_run(self):
        self.guarded("""
            import subprocess, sys
            sleeper = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
            print(sleeper.pid, flush=True)
        """)
        sleeper = int(self.output.splitlines()[0])
        self.addCleanup(os.kill, sleeper, signal.SIGKILL)
        os.kill(sleeper, 0)


if __name__ == "__main__":
    unittest.main()
