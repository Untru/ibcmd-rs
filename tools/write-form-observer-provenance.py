"""Record the exact public diagnostic executable; never read private inputs."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

if len(sys.argv) != 3:
    raise SystemExit('Usage: write-form-observer-provenance.py BINARY OUTPUT_DIRECTORY')
binary = Path(sys.argv[1])
output = Path(sys.argv[2]).resolve()
if binary.is_symlink() or not binary.is_file():
    raise SystemExit('Diagnostic binary must be a plain file')
binary = binary.resolve()
if subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip():
    raise SystemExit('Source checkout is not clean')
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
rustc = subprocess.check_output(['rustc', '-Vv'], text=True)
host = next(line.removeprefix('host: ') for line in rustc.splitlines() if line.startswith('host: '))
output.mkdir(parents=True, exist_ok=False)
target = output / binary.name
shutil.copyfile(binary, target)
binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
if hashlib.sha256(target.read_bytes()).hexdigest() != binary_hash:
    raise SystemExit('Diagnostic staging copy mismatch')
manifest = {
    'schemaVersion': 1,
    'builtCommit': commit,
    'requestedHead': os.environ.get('DIAGNOSTIC_REQUESTED_HEAD'),
    'rustcVv': rustc,
    'target': os.environ.get('CARGO_BUILD_TARGET', host),
    'buildCommand': ['cargo', 'build', '--locked', '--no-default-features', '--example', 'form_field_map_observer'],
    'profile': 'debug',
    'binary': target.name,
    'binaryBytes': target.stat().st_size,
    'binarySHA256': binary_hash,
    'rustflagsSHA256': hashlib.sha256(os.environ.get('RUSTFLAGS', '').encode()).hexdigest(),
    'encodedRustflagsSHA256': hashlib.sha256(os.environ.get('CARGO_ENCODED_RUSTFLAGS', '').encode()).hexdigest(),
    'privateInputUsed': False,
    'purpose': 'read-only structural observation; no parity acceptance',
}
(output / 'provenance.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'builtCommit': commit, 'binarySHA256': binary_hash}))
