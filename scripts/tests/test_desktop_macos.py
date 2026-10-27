import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("desktop_macos", ROOT / "scripts/check-desktop-macos.py")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class DesktopMacosTests(unittest.TestCase):
    def environment(self, system="Darwin", host="aarch64-apple-darwin", target=None, part=None):
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        arguments = ["--part", part] if part else []
        stack.enter_context(patch.object(sys, "argv", ["check-desktop-macos.py", *arguments]))
        stack.enter_context(patch.object(GUARD.platform, "system", return_value=system))
        stack.enter_context(patch.object(GUARD.subprocess, "check_output", return_value=f"host: {host}\n"))
        stack.enter_context(patch.object(GUARD.resource, "getrlimit", return_value=(256, 20480)))
        limit = stack.enter_context(patch.object(GUARD.resource, "setrlimit"))
        env = {"RUSTDOCFLAGS": "--cfg regression_guard"}
        if target is not None:
            env["CARGO_BUILD_TARGET"] = target
        stack.enter_context(patch.dict(os.environ, env, clear=True))
        stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
        run = stack.enter_context(patch.object(GUARD.slow_test_guard, "run"))
        return run, limit

    def test_other_platforms_cannot_satisfy_the_macos_guard(self):
        for settings in [
            {"system": "Linux"},
            {"host": "aarch64-unknown-linux-gnu"},
            {"target": "aarch64-unknown-linux-gnu"},
        ]:
            with self.subTest(settings=settings):
                run, _ = self.environment(**settings)
                with self.assertRaises(ValueError):
                    GUARD.main()
                run.assert_not_called()
                self.doCleanups()

    def test_any_failed_check_stops_the_guard(self):
        for failed_step in range(5):
            with self.subTest(failed_step=failed_step):
                run, _ = self.environment()
                run.side_effect = [None] * failed_step + [subprocess.CalledProcessError(1, "check")]
                with self.assertRaises(subprocess.CalledProcessError):
                    GUARD.main()
                self.assertEqual(run.call_count, failed_step + 1)
                self.doCleanups()

    def test_guard_covers_the_workspace_and_each_part_runs_alone(self):
        run, limit = self.environment()
        GUARD.main()
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands, [
            [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
            ["cargo", "fmt", "--all", "--check"],
            ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
            ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
            ["cargo", "test", "--locked", "--workspace", "--all-features"],
        ])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["cwd"], ROOT)
            self.assertEqual(call.kwargs["env"]["RUSTDOCFLAGS"], "--cfg regression_guard -D warnings")
            self.assertEqual(call.kwargs["env"]["RUST_TEST_THREADS"], "4")
        limit.assert_called_once_with(GUARD.resource.RLIMIT_NOFILE, (10240, 20480))
        self.doCleanups()
        for part in ["lint", "test"]:
            with self.subTest(part=part):
                run, _ = self.environment(part=part)
                GUARD.main()
                self.assertEqual([call.args[0] for call in run.call_args_list], GUARD.PARTS[part])
                self.doCleanups()


if __name__ == "__main__":
    unittest.main()
