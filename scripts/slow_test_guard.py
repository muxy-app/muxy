"""Run a check command and stop it as soon as one of its Rust tests runs for over a minute.

Rust's test harness names a test that is still running after 60 seconds, but only
when it runs tests in parallel. A test binary that prints nothing for as long is
caught too, so a hung test fails the check whatever the harness mode.
"""

import os
import re
import signal
import subprocess
import sys
import tempfile
import time

LIMIT_SECONDS = 60
SLOW_TEST = re.compile(rb"(test .+?) has been running for over \d+ seconds")
TESTS_STARTED = re.compile(rb"running \d+ tests?")
TESTS_FINISHED = re.compile(rb"test result: ")


def run(command, **kwargs):
    """Run the command like subprocess.run(command, check=True, **kwargs), within the test time limit."""
    # A file, unlike a pipe, cannot keep the run waiting on processes the command leaves behind.
    with tempfile.TemporaryFile() as output:
        process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, start_new_session=True, **kwargs)
        try:
            slow_test = watch(process, output.fileno())
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    if slow_test:
        raise TimeoutError(f"Stopped the run because {slow_test} ran for over {LIMIT_SECONDS} seconds")
    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, command)


def watch(process, output):
    """Echo the command's output as it grows and return the first test that runs too long, if any."""
    offset, partial, testing, last_output = 0, b"", False, time.monotonic()
    while True:
        exited = process.poll() is not None
        chunk = os.pread(output, 1 << 16, offset)
        if not chunk:
            if exited:
                return None
            if testing and time.monotonic() - last_output > LIMIT_SECONDS:
                return "a test"
            time.sleep(0.1)
            continue
        offset += len(chunk)
        last_output = time.monotonic()
        sys.stdout.buffer.write(chunk)
        sys.stdout.buffer.flush()
        *lines, partial = (partial + chunk).split(b"\n")
        for line in lines:
            if slow := SLOW_TEST.search(line):
                return slow.group(1).decode(errors="replace")
            if TESTS_STARTED.fullmatch(line):
                testing = True
            elif TESTS_FINISHED.match(line):
                testing = False
