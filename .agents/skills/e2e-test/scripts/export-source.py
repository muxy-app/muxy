#!/usr/bin/env python3
"""Archive the current working tree for an isolated SwiftVM build, without Git credentials."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('repository', type=Path)
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    root = args.repository.resolve()
    archive = args.archive.resolve()
    actual_root = Path(subprocess.check_output(
        ['git', '-C', str(root), 'rev-parse', '--show-toplevel'], text=True).strip()).resolve()
    if root != actual_root:
        parser.error('repository must be the checkout root')
    names = subprocess.check_output([
        'git', '-C', str(root), 'ls-files', '-z', '--cached', '--others', '--exclude-standard'
    ]).split(b'\0')
    files = []
    excluded = {'.git', '.agents', '.claude', '.codex', '.aws', '.ssh', 'target', 'CLAUDE.md'}
    for raw in sorted(set(names)):
        if not raw:
            continue
        relative = Path(os.fsdecode(raw))
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError(f'Unsafe source path: {relative}')
        if any(part in excluded or part.startswith('.env') for part in relative.parts):
            continue
        path = root / relative
        if not path.exists() and not path.is_symlink():
            continue  # A deletion in the working tree must remain deleted in the guest.
        if path.resolve() == archive:
            continue
        if path.is_symlink() and (Path(os.readlink(path)).is_absolute() or not path.resolve().is_relative_to(root)):
            raise ValueError(f'Symlink is not portable inside checkout: {relative}')
        if path.is_dir():
            raise ValueError(f'Export submodule/directory explicitly: {relative}')
        files.append((path, relative))
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, 'w', dereference=False) as output:
        for path, relative in files:
            output.add(path, arcname=str(relative), recursive=False)
    digest = hashlib.sha256()
    with archive.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    print(f'{archive}: {len(files)} files, SHA-256 {digest.hexdigest()}')


if __name__ == '__main__':
    main()
