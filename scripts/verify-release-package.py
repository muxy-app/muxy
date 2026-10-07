#!/usr/bin/env python3
"""Verify a native packaged pair, its installer, and optional matching desktop DMG."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
import zipfile

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'scripts'))
from release import APPS, channel  # noqa: E402


def run(*command, **kwargs):
    print('+ ' + ' '.join(map(str, command)), flush=True)
    return subprocess.run(command, check=True, **kwargs)


def verify(args):
    with tempfile.TemporaryDirectory(prefix='muxy-package-', dir='/tmp') as temporary:
        work = Path(temporary)
        extracted = work / 'extracted'
        extracted.mkdir()
        if args.archive.suffix == '.zip':
            with zipfile.ZipFile(args.archive) as archive:
                assert sorted(archive.namelist()) == ['LICENSE', 'muxy', 'muxy-server']
                for entry in archive.infolist():
                    assert entry.external_attr >> 16 & 0o170000 == 0o100000
                    (extracted / entry.filename).write_bytes(archive.read(entry))
        else:
            with tarfile.open(args.archive) as archive:
                assert sorted(archive.getnames()) == ['LICENSE', 'muxy', 'muxy-server']
                for entry in archive.getmembers():
                    assert entry.isfile()
                    (extracted / entry.name).write_bytes(archive.extractfile(entry).read())
        for name in ('muxy', 'muxy-server'):
            binary = extracted / name
            binary.chmod(0o755)
            run(sys.executable, ROOT / 'scripts/release.py', 'check-build', args.version, binary)
            if platform.system() == 'Darwin':
                assert subprocess.check_output(['lipo', '-archs', binary], text=True).strip() == platform.machine()
                run('codesign', '--verify', '--strict', binary)
                if args.notarized:
                    run('codesign', '--verify', '--requirement', 'notarized', '--check-notarization', binary)
            else:
                target = {'x86_64': 'x86_64', 'aarch64': 'aarch64'}[platform.machine()] + '-unknown-linux-gnu'
                run(sys.executable, ROOT / 'scripts/audit-linux.py', binary, '--target', target)
        if args.dmg:
            mount = work / 'mounted'
            run('hdiutil', 'attach', args.dmg, '-nobrowse', '-readonly', '-mountpoint', mount)
            try:
                app = mount / f'{APPS[channel(args.version)][0]}.app'
                run('codesign', '--verify', '--deep', '--strict', app)
                for name in ('muxy', 'muxy-server'):
                    assert (app / 'Contents/MacOS' / name).read_bytes() == (extracted / name).read_bytes()
                if args.notarized:
                    run('spctl', '--assess', '--type', 'open', '--context', 'context:primary-signature', args.dmg)
            finally:
                run('hdiutil', 'detach', mount)

        # Only download transport is substituted: the production installer reads
        # the exact packaged archive and hashes from this Actions artifact.
        downloads = work / 'downloads'
        downloads.mkdir()
        shutil.copyfile(args.archive, downloads / args.archive.name)
        (downloads / 'SHA256SUMS').write_text(hashlib.sha256(args.archive.read_bytes()).hexdigest() + '  ' + args.archive.name + '\n')
        tools = work / 'tools'
        tools.mkdir()
        curl = tools / 'curl'
        curl.write_text('''#!/bin/sh
set -eu
URL=
OUTPUT=
while [ "$#" -gt 0 ]; do
    case "$1" in https://*) URL=$1; shift ;; -o) OUTPUT=$2; shift 2 ;; *) shift ;; esac
done
case "$URL" in "$VERIFY_URL"/*) ;; *) exit 1 ;; esac
cp "$VERIFY_DOWNLOADS/${URL##*/}" "$OUTPUT"
''')
        curl.chmod(0o755)
        profile = work / 'profile'
        home = work / 'home'
        home.mkdir()
        env = {k: v for k, v in os.environ.items() if not k.startswith('MUXY_')}
        env.update(PATH=f'{tools}:/usr/bin:/bin:/usr/sbin:/sbin', HOME=str(home), SHELL='/bin/sh',
                   MUXY_DIR=str(profile), VERIFY_DOWNLOADS=str(downloads),
                   VERIFY_URL=f'https://github.com/muxy-app/muxy/releases/download/v{args.version}')
        assert not shutil.which('cargo', path=env['PATH'])
        assert not shutil.which('zig', path=env['PATH'])
        destination = work / 'installed with spaces'
        install = ['/bin/sh', str(ROOT / 'scripts/install-muxy.sh'), '--version', args.version,
                   '--install-dir', str(destination)]
        run(*install, env=env)
        assert not profile.exists(), 'Installer started or migrated a server'
        run(sys.executable, ROOT / 'scripts/smoke-headless.py', destination / 'muxy', env=env)
        server = subprocess.Popen([destination / 'muxy-server'], env=env)
        try:
            deadline = time.monotonic() + 10
            socket = profile / 'server.sock'
            while not socket.exists():
                assert server.poll() is None and time.monotonic() < deadline, 'Server failed to start'
                time.sleep(0.025)
            listed = subprocess.check_output([destination / 'muxy', 'project', 'list'], env=env)
            inode = socket.stat().st_ino
            previous = (destination / 'muxy').resolve().parent
            run(*install, '--replace', env=env)
            assert (destination / 'muxy').resolve().parent != previous
            assert server.poll() is None and socket.stat().st_ino == inode
            assert subprocess.check_output([destination / 'muxy', 'project', 'list'], env=env) == listed
        finally:
            if server.poll() is None:
                server.send_signal(signal.SIGTERM)
            try:
                assert server.wait(timeout=10) == 0
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
                raise
        print(json.dumps({'archive': args.archive.name, 'version': args.version, 'native': platform.machine(),
                          'installer': 'passed without Rust/Zig or desktop', 'live_server_preserved': True,
                          'dmg_bytes_match': bool(args.dmg), 'notarization_checked': args.notarized}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('--version', required=True)
    parser.add_argument('--dmg', type=Path)
    parser.add_argument('--notarized', action='store_true')
    options = parser.parse_args()
    options.archive = options.archive.resolve()
    if options.dmg:
        options.dmg = options.dmg.resolve()
    verify(options)
