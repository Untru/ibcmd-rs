"""Validate the pinned public corpus and converter-owned expectations."""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SUBMODULE = 'tests/fixtures/1c-fixtures'
URL = 'https://github.com/Untru/1c-fixtures.git'


def fixture_root(root=ROOT):
    return root / SUBMODULE


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.PIPE).decode('utf-8').strip()


def check_identity(root):
    module = fixture_root(root)
    if not (module / '.git').exists():
        raise ValueError('1c-fixtures is missing: run git submodule update --init --recursive')
    entries = git(root, 'ls-files', '--stage', '--', SUBMODULE).splitlines()
    if len(entries) != 1:
        raise ValueError('1c-fixtures must have exactly one indexed gitlink')
    metadata, path = entries[0].split('\t', 1)
    mode, revision, stage = metadata.split()
    if mode != '160000' or stage != '0' or path != SUBMODULE:
        raise ValueError('1c-fixtures must be a pinned, resolved gitlink')
    if git(module, 'rev-parse', 'HEAD') != revision:
        raise ValueError('1c-fixtures HEAD differs from the pinned gitlink; run git submodule update --init --recursive')
    if git(root, 'config', '-f', '.gitmodules', '--get', 'submodule.1c-fixtures.url') != URL:
        raise ValueError('1c-fixtures must use the public HTTPS repository URL')
    if git(module, 'status', '--porcelain', '--untracked-files=all'):
        raise ValueError('1c-fixtures has modified or untracked files')
    return module


def load(path):
    return json.loads(path.read_text(encoding='utf-8'))


def check_baselines(root, module):
    corpus = module / 'coverage'
    expectations = root / 'tests/fixtures/coverage'
    baselines = sorted(expectations.rglob('*baseline.json'))
    if not baselines:
        raise ValueError('converter coverage baselines are missing')
    variants_checked = 0
    for path in baselines:
        relative = path.parent.relative_to(expectations)
        source = corpus / relative
        manifest_path = source / 'manifest.json'
        if not manifest_path.is_file():
            raise ValueError(f'orphaned baseline set: {relative.as_posix()}')
        manifest = load(manifest_path)
        equal = load(path)['equal']
        if not isinstance(equal, list) or any(not isinstance(v, str) for v in equal) or len(set(equal)) != len(equal):
            raise ValueError(f'invalid equality list: {path}')
        if path.name == 'baseline.json':
            names = {v['name'] for v in manifest['variants'] if v['status'] == 'platform_proven'}
        elif path.name == 'storage-baseline.json':
            names = {p.relative_to(source / 'native').as_posix() for p in (source / 'native').rglob('*') if p.is_file()}
        else:
            raise ValueError(f'unknown baseline kind: {path.name}')
        missing = set(equal) - names
        if missing:
            raise ValueError(f'orphaned baseline entries in {relative.as_posix()}/{path.name}: {sorted(missing)}')
        variants_checked += len(equal)
    probes = load(module / 'coverage-records/source-probes.json')['cases']
    expected = load(root / 'tests/fixtures/coverage-records/source-probe-expectations.json')['cases']
    ids = [p['id'] for p in probes]
    if not ids or len(set(ids)) != len(ids) or set(ids) != set(expected):
        raise ValueError('source probe IDs and converter refusal expectations differ')
    for probe in probes:
        if 'expected_failure' in probe or 'reader_platform' in probe:
            raise ValueError('converter-specific refusal/profile leaked into the corpus')
        contract = expected[probe['id']]
        failure = contract.get('expected_failure')
        parity = contract.get('expected_parity')
        if bool(failure) == bool(parity):
            raise ValueError(f'exactly one refusal or parity expectation is required: {probe["id"]}')
        if failure and not isinstance(failure, str):
            raise ValueError(f'invalid refusal expectation: {probe["id"]}')
        if parity and not (
            parity == 'native_interface_descriptor'
            and probe.get('native_behavior') == 'native_cf'
            and probe.get('operation', {}).get('kind') == 'identity'
            and probe.get('target', '').startswith('Interfaces/')
        ):
            raise ValueError(f'unsupported parity contract: {probe["id"]}')
    return len(baselines), variants_checked, len(ids)


def validate(root=ROOT):
    module = check_identity(root)
    subprocess.run([sys.executable, str(module / 'tools/verify_corpus.py')], check=True)
    counts = check_baselines(root, module)
    print(f'1c-fixtures pinned checkout: PASS; {counts[0]} baseline files, {counts[1]} equal entries, {counts[2]} source probes')


if __name__ == '__main__':
    try:
        validate()
    except (ValueError, KeyError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(f'Coverage corpus validation failed: {error}') from error
