#!/usr/bin/env python3
"""Stage an explicitly supplied, signature-verified distribution WebKit runtime.
No downloads, privilege changes, profile access, or account migration. The caller
verifies the original package signatures before extracting into --source.
"""
import argparse
import json
from pathlib import Path
import re
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parent / 'build/runtime')
args = parser.parse_args()
source = args.source.resolve(strict=True)
output = args.output.absolute()
if output.exists():
    raise SystemExit('Choose a new output directory; an existing runtime is never overwritten.')
if source == output or source in output.parents or output in source.parents:
    raise SystemExit('Source and output must be separate trees.')
receipt = json.loads((source / 'receipt.json').read_text())
if not isinstance(receipt, list) or not receipt or any(row.get('signature') != 'verified by pacman-key' for row in receipt):
    raise SystemExit('An explicit package signature-verification receipt is required.')
for path in (source / 'usr').rglob('*'):
    if path.is_symlink() and not path.resolve().is_relative_to(source):
        raise SystemExit('A runtime symlink escapes the supplied package tree.')
    if path.is_file() and path.stat().st_mode & 0o022:
        raise SystemExit('Runtime files may not be writable by another user.')
required = source / 'usr/lib/webkit2gtk-4.1/WebKitWebProcess'
if not required.is_file():
    raise SystemExit('The supplied runtime has no WebKitWebProcess.')
shutil.copytree(source, output, symlinks=True)
for pc in (output / 'usr/lib/pkgconfig').glob('*.pc'):
    text = re.sub(r'^prefix=.*$', 'prefix=' + str(output / 'usr'), pc.read_text(), flags=re.M)
    text = re.sub(r'^libdir=.*$', 'libdir=' + str(output / 'usr/lib'), text, flags=re.M)
    pc.write_text(text)
print(output)
