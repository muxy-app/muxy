import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = '2.0.0-beta-1234'
FAKE = r'''
import json, os, shutil, subprocess, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
if name == 'uname':
    print(os.environ['TEST_OS'] if args == ['-s'] else os.environ['TEST_ARCH'])
elif name == 'getconf':
    print(os.environ.get('TEST_GLIBC', 'glibc 2.35'))
elif name == 'curl':
    url = next(arg for arg in args if arg.startswith('https://'))
    with open(os.environ['CURL_LOG'], 'a') as log: log.write(url + '\n')
    output = Path(args[args.index('-o') + 1])
    if os.environ.get('DOWNLOAD_FAIL'):
        output.write_bytes(b'partial'); sys.exit(18)
    shutil.copyfile(Path(os.environ['FIXTURES']) / url.rsplit('/', 1)[1], output)
elif name == 'mv':
    if os.environ.get('ACTIVATE_FAIL') and args[-1].endswith('/.muxy/current'): sys.exit(1)
    args = [('-h' if sys.platform == 'darwin' else '-T') if x in ('-h', '-T') else x for x in args]
    status = subprocess.call([os.environ['REAL_MV'], *args])
    if not status and os.environ.get('CHECK_TRANSITION') and args[-1].endswith('/muxy-server'):
        parent = Path(args[-1]).parent
        with open(os.environ['CHECK_TRANSITION'], 'w') as output:
            json.dump({name: (parent / name).read_text() for name in ('muxy', 'muxy-server')}, output)
    sys.exit(status)
'''


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        for tool in ('mkdir', 'rmdir', 'rm', 'cp', 'ln', 'readlink', 'chmod', 'cat', 'awk',
                     'sort', 'cmp', 'sed', 'mktemp', 'tar', 'unzip', 'shasum', 'sha256sum'):
            path = shutil.which(tool)
            if path:
                (self.tools / tool).symlink_to(path)
        for name in ('uname', 'getconf', 'curl', 'mv'):
            path = self.tools / name
            path.write_text(f'#!{sys.executable}\n' + FAKE)
            path.chmod(0o755)
        self.fixtures = self.root / 'fixtures'
        self.fixtures.mkdir()
        self.dest = self.root / "bin with ' spaces"
        self.env = {**os.environ, 'PATH': str(self.tools), 'HOME': str(self.root / 'home'),
                    'TMPDIR': str(self.root), 'TEST_OS': 'Darwin', 'TEST_ARCH': 'arm64',
                    'FIXTURES': str(self.fixtures), 'CURL_LOG': str(self.root / 'curl.log'),
                    'REAL_MV': shutil.which('mv')}
        self.pair = {'muxy': b'client-v1', 'muxy-server': b'server-v1', 'LICENSE': b'license'}
        self.archive()

    def archive(self, members=None, kind=None):
        members = self.pair if members is None else members
        platform = 'macos' if self.env['TEST_OS'] == 'Darwin' else 'linux'
        arch = 'arm64' if self.env['TEST_ARCH'] in ('arm64', 'aarch64') else 'x86_64'
        extension = 'zip' if platform == 'macos' else 'tar.gz'
        self.asset = self.fixtures / f'muxy-{VERSION}-{platform}-{arch}.{extension}'
        if extension == 'zip':
            with zipfile.ZipFile(self.asset, 'w') as archive:
                for name, data in members.items():
                    entry = zipfile.ZipInfo(name)
                    entry.create_system = 3
                    entry.external_attr = ((0o120777 if kind == 'symlink' and name == 'muxy' else 0o100755) << 16)
                    archive.writestr(entry, data)
        else:
            with tarfile.open(self.asset, 'w:gz') as archive:
                for name, data in members.items():
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    entry.mode = 0o755
                    if name == 'muxy' and kind:
                        entry.type = {'symlink': tarfile.SYMTYPE, 'hardlink': tarfile.LNKTYPE,
                                      'fifo': tarfile.FIFOTYPE}[kind]
                        entry.linkname = 'muxy-server'
                        entry.size = 0
                    archive.addfile(entry, io.BytesIO(data))
        (self.fixtures / 'SHA256SUMS').write_text(
            hashlib.sha256(self.asset.read_bytes()).hexdigest() + '  ' + self.asset.name + '\n')

    def install(self, *extra):
        return subprocess.run(['/bin/sh', str(ROOT / 'scripts/install-muxy.sh'), '--version', VERSION,
                               '--install-dir', str(self.dest), *extra], env=self.env,
                              capture_output=True, text=True)

    def assert_pair(self, expected=None):
        for name, data in (expected or self.pair).items():
            path = self.dest / name if name != 'LICENSE' else self.dest / '.muxy/current/LICENSE'
            self.assertEqual(path.read_bytes(), data)
        self.assertEqual((self.dest / 'muxy').resolve().parent, (self.dest / 'muxy-server').resolve().parent)

    def test_all_platform_aliases_install_only_exact_selected_asset(self):
        for system in ('Darwin', 'Linux'):
            for arch in ('arm64', 'aarch64', 'x86_64', 'amd64'):
                with self.subTest(system=system, arch=arch):
                    self.env.update(TEST_OS=system, TEST_ARCH=arch)
                    self.archive()
                    result = self.install('--replace')
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assert_pair()
                    urls = (self.root / 'curl.log').read_text().splitlines()[-2:]
                    self.assertEqual(urls, [f'https://github.com/muxy-app/muxy/releases/download/v{VERSION}/{name}'
                                           for name in (self.asset.name, 'SHA256SUMS')])
                    self.assertIn('Add this directory to PATH:', result.stdout)

    def test_replacement_requires_opt_in_and_switches_one_generation(self):
        self.assertEqual(self.install().returncode, 0)
        old = (self.dest / 'muxy').resolve().parent
        self.assertNotEqual(self.install().returncode, 0)
        self.pair['muxy'] = b'client-v2'
        self.pair['muxy-server'] = b'server-v2'
        self.archive()
        result = self.install('--replace')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_pair()
        self.assertNotEqual((self.dest / 'muxy').resolve().parent, old)
        self.assertEqual((old / 'muxy').read_bytes(), b'client-v1')

    def test_bundle_links_refused_and_explicit_replace_never_changes_bundle(self):
        bundle = self.root / 'Muxy Beta.app'
        bundle.mkdir()
        self.dest.mkdir()
        for name in ('muxy', 'muxy-server'):
            (bundle / name).write_bytes(b'original')
            (self.dest / name).symlink_to(bundle / name)
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('--replace', result.stderr)
        self.assertEqual(self.install('--replace').returncode, 0)
        self.assert_pair()
        self.assertTrue(all(p.read_bytes() == b'original' for p in bundle.iterdir()))

    def test_unmanaged_pair_stays_usable_during_public_link_conversion(self):
        self.dest.mkdir()
        for name in ('muxy', 'muxy-server'):
            (self.dest / name).write_text('old-' + name)
        managed = self.dest / '.muxy'
        stale = managed / 'stale'
        stale.mkdir(parents=True)
        for name in ('muxy', 'muxy-server'):
            (stale / name).write_text('unrelated')
        (managed / 'current').symlink_to('stale')
        transition = self.root / 'transition.json'
        self.env['CHECK_TRANSITION'] = str(transition)
        result = self.install('--replace')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(transition.read_text()), {'muxy': 'old-muxy', 'muxy-server': 'old-muxy-server'})
        self.assert_pair()

    def test_identical_regular_files_are_a_noop(self):
        self.dest.mkdir()
        for name in ('muxy', 'muxy-server'):
            (self.dest / name).write_bytes(self.pair[name])
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.dest / 'muxy').is_symlink())
        self.assertIn('already installed', result.stdout)

    def test_activation_failure_restores_unmanaged_commands_and_managed_pair(self):
        self.dest.mkdir()
        for name in ('muxy', 'muxy-server'):
            (self.dest / name).write_bytes(b'old')
        self.env['ACTIVATE_FAIL'] = '1'
        self.assertNotEqual(self.install('--replace').returncode, 0)
        for name in ('muxy', 'muxy-server'):
            self.assertEqual((self.dest / name).read_bytes(), b'old')
        self.env.pop('ACTIVATE_FAIL')
        self.assertEqual(self.install('--replace').returncode, 0)
        before = (self.dest / 'muxy').resolve()
        self.env['ACTIVATE_FAIL'] = '1'
        self.assertNotEqual(self.install('--replace').returncode, 0)
        self.assertEqual((self.dest / 'muxy').resolve(), before)
        self.assert_pair()

    def test_bad_hash_and_interrupted_download_leave_destination_untouched(self):
        self.asset.write_bytes(b'corrupt')
        self.assertNotEqual(self.install().returncode, 0)
        self.assertFalse(self.dest.exists())
        self.env['DOWNLOAD_FAIL'] = '1'
        self.assertNotEqual(self.install().returncode, 0)
        self.assertFalse(self.dest.exists())
        self.assertFalse(list(self.root.glob('muxy-install.*')))

    def test_traversal_extra_files_and_link_types_are_rejected(self):
        for system in ('Darwin', 'Linux'):
            self.env['TEST_OS'] = system
            for members, kind in (({**self.pair, '../escaped': b'bad'}, None),
                                  ({**self.pair, '/absolute': b'bad'}, None),
                                  ({**self.pair, 'extra': b'bad'}, None), (self.pair, 'symlink')):
                with self.subTest(system=system, kind=kind, members=list(members)):
                    self.archive(members, kind)
                    self.assertNotEqual(self.install().returncode, 0)
                    self.assertFalse(self.dest.exists())
            if system == 'Linux':
                for kind in ('hardlink', 'fifo'):
                    self.archive(kind=kind)
                    self.assertNotEqual(self.install().returncode, 0)
                    self.assertFalse(self.dest.exists())

    def test_unsupported_hosts_and_missing_tools_fail_before_download(self):
        for values in ({'TEST_OS': 'FreeBSD'}, {'TEST_ARCH': 'riscv64'},
                       {'TEST_OS': 'Linux', 'TEST_GLIBC': 'glibc 2.34'},
                       {'TEST_OS': 'Linux', 'TEST_GLIBC': 'musl 1.2'}):
            before = self.env.copy()
            self.env.update(values)
            self.assertNotEqual(self.install().returncode, 0)
            self.assertFalse(self.dest.exists())
            self.assertFalse((self.root / 'curl.log').exists())
            self.env = before
        (self.tools / 'unzip').unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Install unzip', result.stderr)
        self.assertFalse(self.dest.exists())

    def test_missing_checksum_tool_is_actionable(self):
        for tool in ('shasum', 'sha256sum'):
            (self.tools / tool).unlink(missing_ok=True)
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Install sha256sum or shasum', result.stderr)
        self.assertFalse(self.dest.exists())

    def dev_archive(self, members=None, kind=None):
        self.env.update(TEST_OS='Linux', TEST_ARCH='x86_64')
        self.pair = {'muxy': b'dev-client', 'muxy-server': b'dev-server'}
        self.archive(self.pair if members is None else members, kind)

    def install_dev(self, *extra):
        return subprocess.run(
            ['/bin/sh', str(ROOT / 'scripts/install-muxy.sh'), '--dev-archive',
             '--install-dir', str(self.dest), *extra], env=self.env,
            input=self.asset.read_bytes(), capture_output=True)

    def test_development_archive_installs_pair_from_stdin_without_download_tools(self):
        self.dev_archive()
        for tool in ('curl', 'shasum', 'sha256sum'):
            (self.tools / tool).unlink(missing_ok=True)
        result = self.install_dev()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_pair()
        self.assertFalse((self.root / 'curl.log').exists())
        self.assertFalse(list(self.root.glob('muxy-install.*')))

    def test_development_archive_refuses_existing_installation(self):
        self.dev_archive()
        self.dest.mkdir()
        (self.dest / 'muxy').write_bytes(b'existing')
        self.assertNotEqual(self.install_dev().returncode, 0)
        self.assertEqual((self.dest / 'muxy').read_bytes(), b'existing')
        self.assertFalse((self.dest / 'muxy-server').exists())

    def test_development_archive_rejects_wrong_platform_before_install(self):
        self.dev_archive()
        for values in ({'TEST_ARCH': 'arm64'}, {'TEST_OS': 'Darwin'},
                       {'TEST_GLIBC': 'glibc 2.34'}):
            before = self.env.copy()
            self.env.update(values)
            self.assertNotEqual(self.install_dev().returncode, 0)
            self.assertFalse(self.dest.exists())
            self.env = before

    def test_development_archive_rejects_unsafe_or_incomplete_pairs(self):
        for members, kind in (({'../escape': b'bad', 'muxy-server': b'server'}, None),
                              ({'muxy': b'only-one'}, None),
                              ({'muxy': b'', 'muxy-server': b'server'}, None),
                              (None, 'symlink'), (None, 'hardlink'), (None, 'fifo')):
            with self.subTest(members=members, kind=kind):
                self.dev_archive(members, kind)
                self.assertNotEqual(self.install_dev().returncode, 0)
                self.assertFalse((self.dest / 'muxy').exists())
                self.assertFalse((self.dest / 'muxy-server').exists())

    def test_development_archive_activation_failure_rolls_back(self):
        self.dev_archive()
        self.env['ACTIVATE_FAIL'] = '1'
        self.assertNotEqual(self.install_dev().returncode, 0)
        self.assertFalse((self.dest / 'muxy').is_symlink())
        self.assertFalse((self.dest / 'muxy-server').is_symlink())
        self.assertFalse(list((self.dest / '.muxy').glob('pair-*')))


if __name__ == '__main__':
    unittest.main()
