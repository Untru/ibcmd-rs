"""Derive configurations whose root tuple is in an older shape from
platform-built ones, to pin how those shapes read.

Evidence the derivation rests on (8.3.27.2214):
- Src/1Cv8.cf (tuple `{66,…}`, 59 fields) and 1Cv8_обф.cf (the same
  configuration saved by 8.5, `{76,…}`) dump to the identical Configuration.xml.
- MONITOR-TRIAL-2.cf (`{59,…}`, 53 fields) against the same configuration
  re-serialized by 8.3.27 (its native XML loaded back and saved: `{68,…}`, 61
  fields): every shared field equal except the root header (`{2,…}` carrying
  two property states against `{3,…,0,0,<nil>,0}`), the tables in fields 40
  and 51 (shorter lists), field 43; fields 53..60 of the 68 tuple are the
  evidenced all-default reference's.
So an older tuple is a prefix of the 68 one; the rest reads as defaults.

    python make_old_root_fixtures.py

Output: tests/fixtures/external/old_roots/<case>/input.cf where <case> is
  v66, v59            -- from config_compat/c19_e12 (expected: its Configuration.xml)
  v59_form            -- from compat_forms/cf_8_3_18 (expected: its Form.xml)
  ext_v66/input.cfe   -- from extension_roots/values (expected: its Configuration.xml)
  v52                 -- from config_compat/c19_e12, the 8.3.9-mode shape (46 members,
                         older header and identities, field 40 table v13); 8.3.27.2214
                         loads it and dumps c19_e12's Configuration.xml byte for byte
"""
import os, re, struct, sys, zlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8c
from make_closed_module_fixture import refresh_manifest

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = os.path.join(os.path.dirname(HERE), 'tests', 'fixtures', 'external')
NIL = '00000000-0000-0000-0000-000000000000'


def split_fields(text, start):
    """(fields, end) of the brace list opening at text[start]."""
    fields, depth, cur, i, quoted = [], 0, start + 1, start + 1, False
    while i < len(text):
        ch = text[i]
        if quoted:
            if ch == '"':
                if i + 1 < len(text) and text[i + 1] == '"':
                    i += 1
                else:
                    quoted = False
        elif ch == '"':
            quoted = True
        elif ch == '{':
            depth += 1
        elif ch == '}':
            if depth == 0:
                fields.append(text[cur:i].strip())
                return fields, i + 1
            depth -= 1
        elif ch == ',' and depth == 0:
            fields.append(text[cur:i].strip())
            cur = i + 1
        i += 1
    raise ValueError('unterminated list')


def table(field, version, count, tail=''):
    """`{<version>,<count>, first <count> items[, tail]}` of a counted table."""
    items, _ = split_fields(field, 0)
    kept = items[2:2 + count]
    return '{%s,%d,%s%s}' % (version, count, ','.join(kept), (',' + tail) if tail else '')


def configuration_row(entries):
    if 'root' in entries:
        return re.search(r'[0-9a-f]{8}-[0-9a-f-]{27}', entries['root'].decode('utf-8-sig')).group(0)
    # An extension keeps no `root`: its configuration row names itself.
    for name, body in entries.items():
        if len(name) == 36:
            try:
                text = body.decode('utf-8-sig')
            except UnicodeDecodeError:
                continue
            if text.lstrip().startswith('{2,') and ('{%s}' % name) in text[:200] and '{68,' in text:
                return name
    raise ValueError('no configuration row')


def derive(source, case, rewrite, out_name='input.cf', post=None):
    path = os.path.join(FIXTURES, source)
    raw = open(path, 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(path)
    cfg = configuration_row(entries)
    text = entries[cfg].decode('utf-8-sig')
    start = text.index('{68,')
    fields, end = split_fields(text, start)
    assert len(fields) == 61, len(fields)
    fields = rewrite(fields)
    text = text[:start] + '{' + ','.join(fields) + '}' + text[end:]
    if post:
        text = post(text)
    entries[cfg] = ('﻿' + text).encode('utf-8')
    dst = os.path.join(FIXTURES, 'old_roots', case)
    os.makedirs(dst, exist_ok=True)
    container = v8c.write15(entries, storage_version)
    if 'configinfo' in entries and 'root' not in entries:
        entries['configinfo'] = refresh_manifest(entries, container)
        container = v8c.write15(entries, storage_version)
        assert refresh_manifest(entries, container) == entries['configinfo']
    open(os.path.join(dst, out_name), 'wb').write(container)
    print('ok', case, len(fields), 'fields')


def v66(fields):
    # 59 fields; the mobile functionalities table in its version-1 spelling
    # with 33 pairs and the trailing scalar (Src/1Cv8.cf).
    out = ['66'] + fields[1:59]
    out[53] = table(fields[53], 1, 33, '0')
    return out


def v59(fields):
    # 53 fields; header `{2,…}` with two property states and no trailing 0,
    # the flag table (field 51) in version 1 with 24 rows (MONITOR-TRIAL-2.cf).
    out = ['59'] + fields[1:53]
    wrapper, _ = split_fields(out[1], 0)
    header, _ = split_fields(wrapper[1], 0)
    assert header[0] == '3' and header[5:9] == ['0', '0', NIL, '0'], header
    header = ['2'] + header[1:5] + ['0', '2', '7f676314-716a-4d54-8335-a71a3857b21c', '3',
                                    '9dfcabbf-6a7a-48aa-8721-df5e78b367c6', '3', NIL]
    out[1] = '{0,{%s}}' % ','.join(header)
    out[51] = table(fields[51], 1, 24)
    return out


def v52(fields):
    # 46 members (a real configuration in 8.3.9 mode, container version
    # 216): the 59 shape cut shorter, its header `{1,{0,0,<id>},…,0,0}` and
    # the flag table of field 40 in its version 13 (13 flags, all off, in the
    # order that configuration keeps). 8.3.27.2214 loads the result and dumps
    # c19_e12's own Configuration.xml byte for byte (with field 40 left in
    # version 28 it crashes).
    out = ['52'] + fields[1:46]
    out[40] = '{13,' + ','.join('{{"#",e4c53f94-e5f7-4a34-8c10-218bd811cae1,%d},{"B",0}}' % k
                                for k in (0, 3, 11, 1, 2, 10, 4, 5, 6, 7, 8, 9, 12)) + '}'
    wrapper, _ = split_fields(out[1], 0)
    header, _ = split_fields(wrapper[1], 0)
    assert header[0] == '3' and header[5:9] == ['0', '0', NIL, '0'], header
    object_id = re.search(r'[0-9a-f]{8}-[0-9a-f-]{27}', header[1]).group(0)
    out[1] = '{0,{1,{0,0,%s},%s,0,0}}' % (object_id, ','.join(header[2:5]))
    return out


def v52_identities(text):
    # The same root spells its sections' object identities `{0,0,<id>}`.
    return re.sub(r'\{1,0,([0-9a-f]{8}-[0-9a-f-]{27})\}', r'{0,0,\1}', text)


def main():
    derive('config_compat/c19_e12/input.cf', 'v66', v66)
    derive('config_compat/c19_e12/input.cf', 'v59', v59)
    derive('compat_forms/cf_8_3_18/input.cf', 'v59_form', v59)
    # An extension root in the 66 shape (ИР 7.77 keeps `{66,…}`); the writer
    # reads members up to 49 only. Expected: extension_roots/values.
    derive('extension_roots/values/input.cfe', 'ext_v66', lambda fields: ['66'] + fields[1:59],
           'input.cfe')
    derive('config_compat/c19_e12/input.cf', 'v52', v52, post=v52_identities)


if __name__ == '__main__':
    main()
