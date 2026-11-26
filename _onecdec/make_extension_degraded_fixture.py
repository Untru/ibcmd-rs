"""Clean-room extensions with their root controlling a property no probe has
named (the DefaultLanguage uuid replaced by an unknown one): the extension
writer must not guess it and must not fail the whole export.

    python make_extension_degraded_fixture.py

Output: tests/fixtures/external/extension_roots/<case>/input.cfe for
  unknown_property          -- from test_extension
  unknown_property_compat14 -- from extension_roots/spellings (extension
                               compatibility 8.3.14): the degraded root keeps
                               the stored compatibility, not the edition
"""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import v8c
from make_closed_module_fixture import refresh_manifest

REPO = os.path.dirname(HERE)
FIXTURES = os.path.join(REPO, 'tests', 'fixtures', 'external')
KNOWN = b'15e3462b-bc9b-40cd-9d9f-dc0922f84560'
UNKNOWN = b'ffffffff-0000-4000-8000-000000000001'
ROOT = '00000000-0000-0000-0000-000000000014'


def derive(source, case):
    entries = v8c.read(os.path.join(FIXTURES, source))
    root = entries[ROOT]
    assert root.count(KNOWN) == 1, source
    entries[ROOT] = root.replace(KNOWN, UNKNOWN)
    dst = os.path.join(FIXTURES, 'extension_roots', case)
    os.makedirs(dst, exist_ok=True)
    container = v8c.write15(entries, deflate_all=True)
    entries['configinfo'] = refresh_manifest(entries, container)
    container = v8c.write15(entries, deflate_all=True)
    assert refresh_manifest(entries, container) == entries['configinfo']
    open(os.path.join(dst, 'input.cfe'), 'wb').write(container)
    print('ok', dst)


derive(os.path.join('test_extension', 'input.cfe'), 'unknown_property')
derive(os.path.join('extension_roots', 'spellings', 'input.cfe'), 'unknown_property_compat14')
