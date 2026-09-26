import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("fixtures", ROOT / "scripts/protocol_fixtures.py")
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)

V2 = "crates/muxy-protocol/tests/fixtures/v2"


class ProtocolFixtureTests(unittest.TestCase):
    def test_only_additions_are_free(self):
        diff = "\n".join([
            f"A\t{V2}/request-0011223344556677.bin",
            f"M\t{V2}/reply-8899aabbccddeeff.bin",
            f"D\t{V2}/frame-0123456789abcdef.bin",
            f"R100\t{V2}/hello-1111111111111111.bin\t{V2}/hello-2222222222222222.bin",
            "M\tcrates/muxy-protocol/tests/fixtures/project-logo.png",
        ])
        self.assertEqual(fixtures.rewritten(diff), [
            f"{V2}/reply-8899aabbccddeeff.bin",
            f"{V2}/frame-0123456789abcdef.bin",
            f"{V2}/hello-1111111111111111.bin",
        ])

    def test_current_version_is_read_from_source(self):
        self.assertEqual(fixtures.current_version("pub const CURRENT: Version = V3;\n"), 3)
        self.assertIsNone(fixtures.current_version("pub const SUPPORTED: &[Version] = &[V1];\n"))
        source = (ROOT / fixtures.VERSION).read_text()
        self.assertTrue((ROOT / fixtures.FIXTURES / f"v{fixtures.current_version(source)}").is_dir())

    def test_unknown_or_new_branch_bases_are_skipped(self):
        fixtures.append_only("")
        fixtures.append_only("0" * 40)


if __name__ == "__main__":
    unittest.main()
