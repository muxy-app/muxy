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
SPEC = importlib.util.spec_from_file_location("desktop_linux", ROOT / "scripts/check-desktop-linux.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)

TARGET = "aarch64-unknown-linux-gnu"
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()


class DesktopLinuxTests(unittest.TestCase):
    def environment(self, system="Linux", machine="aarch64", host=TARGET, zig="/opt/zig/zig", env=None, part=None):
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        arguments = ["--part", part] if part else []
        stack.enter_context(patch.object(sys, "argv", ["check-desktop-linux.py", "--target", TARGET, *arguments]))
        stack.enter_context(patch.object(CHECK.platform, "system", return_value=system))
        stack.enter_context(patch.object(CHECK.platform, "machine", return_value=machine))
        stack.enter_context(patch.object(CHECK.subprocess, "check_output", return_value=f"host: {host}\n"))
        stack.enter_context(patch.object(CHECK.shutil, "which", return_value=zig))
        stack.enter_context(patch.dict(
            os.environ, {"PATH": "/usr/bin", "RUSTDOCFLAGS": "--cfg regression_guard", **(env or {})}, clear=True))
        stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
        return stack.enter_context(patch.object(CHECK.slow_test_guard, "run"))

    def test_only_the_requested_native_linux_host_with_a_real_zig_can_run_the_check(self):
        for settings in [
            {"system": "Darwin"},
            {"machine": "x86_64"},
            {"host": "x86_64-unknown-linux-gnu"},
            {"env": {"CARGO_BUILD_TARGET": "x86_64-unknown-linux-gnu"}},
            {"zig": None},
            {"zig": str(ROOT / "scripts/zig/zig")},
        ]:
            with self.subTest(settings=settings):
                run = self.environment(**settings)
                with self.assertRaises(ValueError):
                    CHECK.main()
                run.assert_not_called()
                self.doCleanups()

    def test_any_failed_check_stops_the_run(self):
        for failed_step in range(5):
            with self.subTest(failed_step=failed_step):
                run = self.environment()
                run.side_effect = [None] * failed_step + [subprocess.CalledProcessError(1, "check")]
                with self.assertRaises(subprocess.CalledProcessError):
                    CHECK.main()
                self.assertEqual(run.call_count, failed_step + 1)
                self.doCleanups()

    def test_check_covers_the_whole_workspace_through_the_baseline_zig_wrapper(self):
        run = self.environment()
        CHECK.main()
        self.assertEqual([call.args[0] for call in run.call_args_list], [
            [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
            ["cargo", "fmt", "--all", "--check"],
            ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
            ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
            ["cargo", "test", "--locked", "--workspace", "--all-features"],
        ])
        for call in run.call_args_list:
            env = call.kwargs["env"]
            self.assertEqual(call.kwargs["cwd"], ROOT)
            self.assertEqual(env["MUXY_ZIG"], "/opt/zig/zig")
            self.assertEqual(env["PATH"], f"{ROOT / 'scripts/zig'}{os.pathsep}/usr/bin")
            self.assertEqual(env["RUSTDOCFLAGS"], "--cfg regression_guard -D warnings")
        self.doCleanups()
        for part in ["lint", "test"]:
            with self.subTest(part=part):
                run = self.environment(part=part)
                CHECK.main()
                self.assertEqual([call.args[0] for call in run.call_args_list], CHECK.PARTS[part])
                self.doCleanups()

    def test_ci_runs_desktop_checks_on_both_architectures_beside_the_headless_and_macos_jobs(self):
        for expected in [
            "python3 scripts/check-desktop-macos.py",
            'python3 scripts/check-headless.py --target "${{ matrix.target }}"',
            'python3 scripts/check-desktop-linux.py --target "${{ matrix.target }}"',
            "runner: ubuntu-22.04\n",
            "runner: ubuntu-22.04-arm\n",
            "runner: ubuntu-24.04\n",
            "runner: ubuntu-24.04-arm\n",
            "part: [lint, test]",
            '--part "${{ matrix.part }}"',
        ]:
            self.assertIn(expected, WORKFLOW)
        self.assertNotIn("continue-on-error", WORKFLOW)


if __name__ == "__main__":
    unittest.main()
