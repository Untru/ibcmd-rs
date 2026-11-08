import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description='Live negative controls for the ICU locale measurement')
parser.add_argument('--icu-root', required=True, type=Path)
parser.add_argument('--probe', required=True, type=Path)
args = parser.parse_args()
path = Path(__file__).with_name('inspect-filedb-icu46-locales.py')
spec = importlib.util.spec_from_file_location('locale_measure', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
runtime = module.ICU.ICU46(args.icu_root)
probe = args.probe
original_run = module.subprocess.run
passed = []


def expect_failure(action, fragment):
    try:
        action()
    except RuntimeError as error:
        assert fragment in str(error), str(error)
    else:
        raise AssertionError('Corrupted measurement was accepted: ' + fragment)


def corrupt_second(kind):
    calls = 0

    def changed(*args, **kwargs):
        nonlocal calls
        calls += 1
        result = original_run(*args, **kwargs)
        if calls == 2:
            data = result.stdout
            if kind == 'key':
                assert data.startswith(b'OK\t')
                data = data[:3] + (b'0' if data[3:4] != b'0' else b'1') + data[4:]
            elif kind == 'refusal':
                assert b'REFUSED\n' in data
                data = data.replace(b'REFUSED\n', b'OK\t' + b'00' * 392 + b'\n', 1)
            elif kind == 'count':
                data = b''.join(data.splitlines(keepends=True)[:-1])
            result = subprocess.CompletedProcess(result.args, result.returncode, data, result.stderr)
        return result

    return changed


for kind, fragment in (
    ('key', 'Candidate differs from pinned ICU'),
    ('refusal', 'An unmeasured locale was not refused'),
    ('count', 'Probe response count differs'),
):
    module.subprocess.run = corrupt_second(kind)
    try:
        expect_failure(lambda: module.measure(runtime, probe), fragment)
        passed.append(kind)
    finally:
        module.subprocess.run = original_run

original_collator = runtime.collator


def wrong_locale(locale):
    collator, metadata = original_collator(locale)
    if locale == 'tr_TR':
        metadata['valid_locale'] = 'ru_RU'
    return collator, metadata


runtime.collator = wrong_locale
try:
    expect_failure(lambda: module.measure(runtime, probe), 'Requested locale resolved to another language')
    passed.append('locale_metadata')
finally:
    runtime.collator = original_collator

expected_hash = module.ICU.DLL_SHA256['icudt46.dll']
module.ICU.DLL_SHA256['icudt46.dll'] = '0' * 64
try:
    expect_failure(lambda: module.ICU.ICU46(args.icu_root), 'Unrecognized ICU DLL')
    passed.append('data_hash')
finally:
    module.ICU.DLL_SHA256['icudt46.dll'] = expected_hash

print(json.dumps({'negative_controls_passed': passed, 'count': len(passed)}))
