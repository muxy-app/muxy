import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / (name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


HEADLESS = module("check-headless")
AUDIT = module("audit-linux")
SMOKE = module("smoke-headless")


class HeadlessTests(unittest.TestCase):
    @unittest.skipIf(sys.platform == "win32", "The Linux smoke test checks POSIX file modes")
    def test_smoke_waits_for_the_restarted_servers_socket_not_its_retained_catalog(self):
        profile = None

        def start(command, *, env, **kwargs):
            nonlocal profile
            profile = Path(env["MUXY_DIR"])
            server = Mock()
            server.poll.return_value = None
            server.wait.return_value = 0
            server.send_signal.side_effect = lambda _: (profile / "server.sock").unlink(missing_ok=True)
            return server

        def ready(_):
            for name in ("sessions/catalog.json", "server.log", "server.toml", "server.sock"):
                path = profile / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch(mode=0o600)

        def output(command, **kwargs):
            if command[1].startswith("--"):
                return "{}"
            self.assertTrue((profile / "server.sock").exists(), "CLI used before the owned server was ready")
            return "Home\n"

        with patch.object(SMOKE.subprocess, "Popen", side_effect=start) as spawn, \
                patch.object(SMOKE.subprocess, "check_output", side_effect=output), \
                patch.object(SMOKE.time, "sleep", side_effect=ready) as wait:
            SMOKE.smoke(Path("/unused/muxy"))
        self.assertEqual(spawn.call_count, 2)
        self.assertEqual(wait.call_count, 2)

    @unittest.skipIf(sys.platform == "win32", "The Zig wrapper is a POSIX shell script")
    def test_zig_build_uses_baseline_cpu_and_preserves_other_arguments(self):
        with tempfile.TemporaryDirectory(prefix="zig test ") as temporary:
            zig = Path(temporary) / "real zig"
            zig.write_text(f"#!{sys.executable}\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n")
            zig.chmod(0o755)
            for arguments, expected in [
                (["version"], ["version"]),
                (["build", "-Dother=true"], ["build", "-Dother=true"]),
                (["build", "-Demit-lib-vt=true", "--prefix", "a path"],
                 ["build", "-Demit-lib-vt=true", "--prefix", "a path", "-Dcpu=baseline"]),
            ]:
                output = subprocess.check_output(
                    [ROOT / "scripts/zig/zig", *arguments],
                    env={**os.environ, "MUXY_ZIG": str(zig)}, text=True,
                )
                self.assertEqual(json.loads(output), expected)

    def test_metadata_closure_includes_transitive_build_and_test_packages(self):
        def package(name, *dependencies):
            return {"id": name, "name": name, "dependencies": [{"name": dep} for dep in dependencies]}
        metadata = {"workspace_members": ["muxy-cli", "muxy-server", "runtime", "protocol", "test-helper", "unused"],
                    "packages": [package("muxy-cli", "runtime"), package("muxy-server", "runtime"), package("runtime", "protocol", "test-helper"),
                                 package("protocol"), package("test-helper", "protocol"), package("unused")]}
        self.assertEqual(HEADLESS.packages(metadata), ["muxy-cli", "muxy-server", "protocol", "runtime", "test-helper"])
        bad = copy.deepcopy(metadata)
        bad["packages"][2]["dependencies"].append({"name": "gpui"})
        with self.assertRaisesRegex(ValueError, "desktop dependency"):
            HEADLESS.packages(bad)

    def test_elf_audit_rejects_wrong_architecture_libraries_and_glibc_floor(self):
        header = "Class: ELF64\nData: 2's complement, little endian\nMachine: AArch64\n"
        segments = "[Requesting program interpreter: /lib/ld-linux-aarch64.so.1]"
        dynamic = "0x1 (NEEDED) Shared library: [libc.so.6]\n"
        versions = "Name: GLIBC_2.17\nName: GLIBC_2.35\n"
        linked = "libc.so.6 => /lib/aarch64-linux-gnu/libc.so.6 (0x0)"
        good = [header, segments, dynamic, versions, linked, "aarch64-unknown-linux-gnu"]
        AUDIT.audit(*good)
        loader = good.copy()
        loader[2] += "0x1 (NEEDED) Shared library: [ld-linux-aarch64.so.1]\n"
        AUDIT.audit(*loader)
        for index, value in [
            (0, header.replace("AArch64", "ARM")),
            (0, header.replace("ELF64", "ELF32")),
            (1, "[Requesting program interpreter: /tmp/loader]"),
            (2, dynamic + "0x1 (NEEDED) Shared library: [libghostty-vt.so]\n"),
            (2, dynamic + "0x1 (NEEDED) Shared library: [ld-linux-x86-64.so.2]\n"),
            (2, dynamic + "0x2 (RUNPATH) Library runpath: [/build]"),
            (3, versions + "Name: GLIBC_2.36\n"),
            (3, versions + "Name: GLIBC_PRIVATE\n"),
            (3, ""),
            (4, "libc.so.6 => not found"),
            (4, "libc.so.6 => /build/libc.so.6 (0x0)"),
        ]:
            with self.subTest(index=index, value=value):
                bad = good.copy()
                bad[index] = value
                with self.assertRaises(ValueError):
                    AUDIT.audit(*bad)


if __name__ == "__main__":
    unittest.main()
