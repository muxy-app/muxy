import base64
import json
import os
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ElementTree
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SIGNATURE = base64.b64encode(bytes(range(64))).decode()
SPARKLE = "{http://www.andymatuschak.org/xml-namespaces/sparkle}"

FAKE_SIGN_UPDATE = r'''
import json, os, sys
with open(os.environ["TOOL_LOG"], "a") as log:
    log.write(json.dumps({"args": sys.argv[1:], "key": sys.stdin.read()}) + "\n")
if "--verify" in sys.argv:
    sys.exit(int(os.environ.get("VERIFY_EXIT", "0")))
print(os.environ["SIGNATURE"])
'''


class SparkleAppcastTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tool = self.root / "sign_update"
        self.tool.write_text(f"#!{sys.executable}\n" + FAKE_SIGN_UPDATE)
        self.tool.chmod(0o755)
        self.dmg = self.root / "Muxy-2.0.0-arm64.dmg"
        self.dmg.write_bytes(b"signed dmg")
        self.output = self.root / "appcast-arm64.xml"
        self.log = self.root / "tool.jsonl"
        self.env = {**os.environ, "TOOL_LOG": str(self.log), "SIGNATURE": SIGNATURE,
                    "SPARKLE_PRIVATE_KEY": "private-key"}

    def write(self, version="2.0.0", build="1150", dmg=None, **env):
        return subprocess.run(
            [sys.executable, str(ROOT / "scripts/sparkle-appcast.py"), version, build,
             str(dmg or self.dmg), str(self.output), "--repository", "muxy-app/muxy",
             "--sign-update", str(self.tool)],
            env={**self.env, **env}, capture_output=True, text=True,
        )

    def test_appcast_offers_the_signed_dmg_to_older_builds(self):
        result = self.write()
        self.assertEqual(result.returncode, 0, result.stderr)
        item = ElementTree.parse(self.output).find("channel/item")
        self.assertEqual(item.find(f"{SPARKLE}version").text, "1150")
        self.assertEqual(item.find(f"{SPARKLE}shortVersionString").text, "2.0.0")
        self.assertEqual(item.find(f"{SPARKLE}minimumSystemVersion").text, "14.0")
        enclosure = item.find("enclosure")
        self.assertEqual(enclosure.get("url"),
                         "https://github.com/muxy-app/muxy/releases/download/v2.0.0/Muxy-2.0.0-arm64.dmg")
        self.assertEqual(enclosure.get(f"{SPARKLE}edSignature"), SIGNATURE)
        self.assertEqual(enclosure.get("length"), str(len(b"signed dmg")))
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertEqual([call["key"] for call in calls], ["private-key", "private-key"])
        self.assertEqual(calls[1]["args"][-2:], [str(self.dmg), SIGNATURE])

    def test_only_signed_stable_dmgs_are_offered(self):
        other = self.root / "Muxy-2.0.1-arm64.dmg"
        other.write_bytes(b"dmg")
        for arguments in ({"version": "2.0.0-beta.1150"}, {"build": "0"}, {"dmg": other}):
            with self.subTest(**arguments):
                self.assertNotEqual(self.write(**arguments).returncode, 0)
        for env in ({"SPARKLE_PRIVATE_KEY": ""}, {"SIGNATURE": "ERROR! not a key"}, {"VERIFY_EXIT": "1"}):
            with self.subTest(**env):
                self.assertNotEqual(self.write(**env).returncode, 0)
                self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
