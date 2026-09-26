import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
VERSION = '2.0.0-beta-1234'
FAKE = r'''
import json, os, shutil, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ['TOOL_LOG'], 'a') as log: log.write(json.dumps([name, *args]) + '\n')
arm = os.environ['TEST_ARCH'] == 'arm64'
if name == 'uname': print('Linux' if args == ['-s'] else ('aarch64' if arm else 'x86_64'))
elif name == 'getconf': print(os.environ.get('TEST_GLIBC', 'glibc 2.35'))
elif name == 'objcopy': shutil.copyfile(args[-2], args[-1])
elif name == 'readelf':
    loader = '/lib/ld-linux-aarch64.so.1' if arm else '/lib64/ld-linux-x86-64.so.2'
    if args[0] == '-hW': print("Class: ELF64\nData: 2's complement, little endian\nMachine: " + ('AArch64' if arm else 'Advanced Micro Devices X86-64'))
    elif args[0] == '-lW': print('[Requesting program interpreter: ' + loader + ']')
    elif args[0] == '-dW': print('(NEEDED) Shared library: [' + os.environ.get('TEST_LIBRARY', 'libc.so.6') + ']')
    elif args[0] == '-VW': print('Name: GLIBC_2.34')
    else: sys.exit(1)
elif name == 'ldd': print('libc.so.6 => /lib/libc.so.6 (0x0)')
elif name not in ('cargo', 'strip', 'zig'): sys.exit('unexpected tool ' + name)
'''


class LinuxReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='linux package ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for relative in ('scripts/build-cli-linux.sh', 'scripts/beta_release.py',
                         'scripts/audit-linux.py', 'scripts/zig/zig', 'crates/muxy-protocol/src/build.rs',
                         'crates/muxy-protocol/src/version.rs', 'LICENSE', 'crates/muxy-server/src/detection/THIRD_PARTY.md', 'crates/muxy-server/src/detection/LICENSE-herdr'):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, path)
        sys.path.insert(0, str(ROOT / 'scripts'))
        from beta_release import build_metadata
        build = build_metadata(ROOT)
        self.pair = {}
        for name in ('muxy', 'muxy-server'):
            self.pair[name] = f'#!{sys.executable}\n# {name}\nprint({json.dumps(json.dumps({"version": VERSION, **build}))})\n'.encode()
        for arch in ('aarch64', 'x86_64'):
            binary_dir = self.root / f'target/{arch}-unknown-linux-gnu/release'
            binary_dir.mkdir(parents=True)
            for name, data in self.pair.items():
                (binary_dir / name).write_bytes(data)
        tools = self.root / 'tools'
        tools.mkdir()
        for name in ('uname', 'getconf', 'objcopy', 'readelf', 'ldd', 'cargo', 'strip', 'zig'):
            path = tools / name
            path.write_text(f'#!{sys.executable}\n' + FAKE)
            path.chmod(0o755)
        self.log = self.root / 'tools.jsonl'
        self.env = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'], 'TOOL_LOG': str(self.log), 'COPYFILE_DISABLE': '1'}

    def build(self, arch='arm64', **overrides):
        return subprocess.run(['bash', str(self.root / 'scripts/build-cli-linux.sh'), arch, VERSION],
                              env={**self.env, 'TEST_ARCH': arch, **overrides}, capture_output=True, text=True)

    def test_both_native_archives_contain_audited_pair_and_license_with_symbols(self):
        for arch in ('arm64', 'x86_64'):
            with self.subTest(arch=arch):
                result = self.build(arch)
                self.assertEqual(result.returncode, 0, result.stderr)
                output = self.root / f'target/beta/{VERSION}/linux-{arch}'
                with tarfile.open(output / f'muxy-{VERSION}-linux-{arch}.tar.gz') as archive:
                    self.assertEqual(archive.getnames(), ['muxy', 'muxy-server', 'LICENSE'])
                    self.assertIn(b'Apache License', archive.extractfile('LICENSE').read())
                    for name, data in self.pair.items():
                        self.assertEqual(archive.extractfile(name).read(), data)
                        self.assertEqual(archive.getmember(name).mode, 0o755)
                        self.assertTrue((output / f'{name}-audit.txt').is_file())
                self.assertTrue((output / f'muxy-{VERSION}-linux-{arch}-symbols.tar.gz').is_file())
                calls = [json.loads(line) for line in self.log.read_text().splitlines()]
                builds = [call for call in calls if call[:2] == ['cargo', 'build']]
                self.assertTrue(all('muxy-cli' in call and 'muxy-server' in call and 'muxy-app' not in call for call in builds))
                before = self.log.read_text()
                self.assertNotEqual(self.build(arch).returncode, 0)
                self.assertEqual([line for line in self.log.read_text()[len(before):].splitlines() if 'cargo' in line], [])

    def test_bad_floor_or_dependency_leaves_no_published_output(self):
        output = self.root / f'target/beta/{VERSION}/linux-arm64'
        for override in ({'TEST_GLIBC': 'glibc 2.39'}, {'TEST_LIBRARY': 'libghostty-vt.so'}):
            result = self.build(**override)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists())
            self.assertFalse(list(output.parent.glob('.linux-*')))
        self.assertEqual(self.build().returncode, 0)


if __name__ == '__main__':
    unittest.main()
