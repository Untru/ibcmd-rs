"""Derive the configinfo-mismatch fixture from `test_extension`: the same
extension container plus one entry its `configinfo` does not list. No
platform run: the case is what an export must survive, not what the platform
writes -- the rest of the tree is still `test_extension/expected`.

    python make_configinfo_mismatch_fixture.py

Output: tests/fixtures/external/configinfo_mismatch/input.cfe
"""
import os, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8c

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXTRA = '11111111-1111-4111-8111-111111111111'


def main():
    src = os.path.join(REPO, 'tests', 'fixtures', 'external', 'test_extension', 'input.cfe')
    raw = open(src, 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(src)
    assert EXTRA not in entries
    entries[EXTRA] = '﻿{0}'.encode('utf-8')
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'configinfo_mismatch')
    os.makedirs(dst, exist_ok=True)
    open(os.path.join(dst, 'input.cfe'), 'wb').write(v8c.write15(entries, storage_version))
    print('ok', sorted(entries))


if __name__ == '__main__':
    main()
