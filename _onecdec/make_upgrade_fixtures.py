"""Exchange plans stored in older record versions, and the objects typed by them.

    python make_upgrade_fixtures.py [--explore]

1. The clean-room base configuration plus an exchange plan (one attribute, the
   seven standard attributes older plans keep -- no ExchangeDate) and a constant,
   a session parameter and a defined type of its reference type. The platform
   (8.3.27.2214) loads the XML, saves `current.cf` (the plan as `{37,…}`, 51
   members) and dumps the expected XML.
2. `v35.cf` / `v36.cf`: `current.cf` with the plan's record rewritten into the
   older shapes real configurations keep (`{35,…}` 49 members;
   `{36,…}` 50), i.e. the inverse of what the platform does
   to them on load (pairs old / re-serialized, `reserialize.py`, 51 plans in
   seven real configurations):
     plan 35 -> 37: members 49, 50 appended as `0`, `1`; 36 -> 37: member 50 `1`
     attribute wrapper `{3,…}` 5 members -> `{4,…,0,{1,<nil>}}` 7
     standard attribute bag `{13,24,…}` -> `{14,25,…}` with the pair
       3b10624f-…, {"#",502b7765-…,{502b7765-…,0}} after the sixth pair
   Each derived .cf is then loaded by the platform and dumped again: the dump
   must equal the dump of `current.cf` (asserted here) -- the platform itself
   reads the older shape as the expected XML.

3. `--old-headers` (from the committed current.cf, without re-running step 1):
   `old_headers.cf` -- every header block `{3,{1,0,u},N,S,C,0,0,<nil>,0}`
   spelled `{1,{0,0,u},N,S,C,0,0}` and the root's section identities
   `{0,0,id}` (as a configuration saved in 8.3.9 mode keeps them);
   `old_headers_2.cf` -- every header spelled `{2,{1,0,u},N,S,C,0,0,<nil>}`
   (8.3.14 mode). The platform dumps both to the expected XML (asserted).

4. `--constant-v14`: `constant_v14.cf` -- the constant stored as `{14,…}` (its
   first 12 members, no ValueKey type ids). The platform loads it and dumps
   ValueKey ids of its own (UUIDv5-shaped, the same on every load, not the
   original's): the exporter cannot know them and must refuse the file.

Output: tests/fixtures/external/upgrade/exchange_plan/{v35.cf, v36.cf, current.cf,
old_headers.cf, old_headers_2.cf, constant_v14.cf, expected/...}
"""
import os, re, shutil, struct, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8c, v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src', 'cfe_base')
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
DST = os.path.join(REPO, 'tests', 'fixtures', 'external', 'upgrade', 'exchange_plan')
NIL = '00000000-0000-0000-0000-000000000000'

HEAD = ('<?xml version="1.0" encoding="UTF-8"?>\n<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" '
        'xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" '
        'xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" '
        'xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">\n')
PLAN = 'ОбменТест'
PLAN_UUID = '6d3b0c2e-1f4a-4c55-9a0e-2b7f5d1e8a01'
REF = 'cfg:ExchangePlanRef.' + PLAN


def std(name, fill_checking='DontCheck'):
    return ('<xr:StandardAttribute name="%s"><xr:LinkByType/><xr:FillChecking>%s</xr:FillChecking>'
            '<xr:MultiLine>false</xr:MultiLine><xr:FillFromFillingValue>false</xr:FillFromFillingValue>'
            '<xr:CreateOnInput>Auto</xr:CreateOnInput><xr:TypeReductionMode>TransformValues</xr:TypeReductionMode>'
            '<xr:MaxValue xsi:nil="true"/><xr:ToolTip/><xr:ExtendedEdit>false</xr:ExtendedEdit><xr:Format/>'
            '<xr:ChoiceForm/><xr:QuickChoice>Auto</xr:QuickChoice><xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>'
            '<xr:EditFormat/><xr:PasswordMode>false</xr:PasswordMode><xr:DataHistory>Use</xr:DataHistory>'
            '<xr:MarkNegatives>false</xr:MarkNegatives><xr:MinValue xsi:nil="true"/><xr:Synonym/><xr:Comment/>'
            '<xr:FullTextSearch>Use</xr:FullTextSearch><xr:ChoiceParameterLinks/><xr:FillValue xsi:nil="true"/>'
            '<xr:Mask/><xr:ChoiceParameters/></xr:StandardAttribute>') % (name, fill_checking)


def uid(n):
    return 'a0b1c2d3-e4f5-4a6b-8c7d-%012x' % n


def generated(pairs, start):
    """`<InternalInfo>` of (name, category) generated types with fixed ids."""
    return '<InternalInfo>%s</InternalInfo>' % ''.join(
        '<xr:GeneratedType name="%s" category="%s"><xr:TypeId>%s</xr:TypeId><xr:ValueId>%s</xr:ValueId>'
        '</xr:GeneratedType>' % (name, category, uid(start + 2 * i), uid(start + 2 * i + 1))
        for i, (name, category) in enumerate(pairs))


PLAN_INFO = generated([('ExchangePlan%s.%s' % (c, PLAN), c) for c in ('Object', 'Ref', 'Selection', 'List', 'Manager')],
                      0x10).replace('<InternalInfo>', '<InternalInfo><xr:ThisNode>%s</xr:ThisNode>' % uid(0x1f))
CONST_INFO = generated([('ConstantManager.КонстантаУзел', 'Manager'), ('ConstantValueManager.КонстантаУзел', 'ValueManager'),
                        ('ConstantValueKey.КонстантаУзел', 'ValueKey')], 0x20)
DEFINED_INFO = generated([('DefinedType.ТипУзла', 'DefinedType')], 0x30)

OBJECTS = {
    'ExchangePlans/%s.xml' % PLAN: HEAD + (
        '<ExchangePlan uuid="%s">' + PLAN_INFO + '<Properties><Name>%s</Name><Synonym/><Comment/>'
        '<CodeLength>9</CodeLength><DescriptionLength>25</DescriptionLength>'
        '<StandardAttributes>%s</StandardAttributes>'
        '<DataLockControlMode>Managed</DataLockControlMode></Properties>'
        '<ChildObjects><Attribute uuid="0c1e2f3a-4b5c-4d6e-8f70-8192a3b4c5d6"><Properties><Name>Очередь</Name>'
        '<Synonym/><Comment/><Type><v8:Type>xs:decimal</v8:Type><v8:NumberQualifiers><v8:Digits>10</v8:Digits>'
        '<v8:FractionDigits>0</v8:FractionDigits><v8:AllowedSign>Any</v8:AllowedSign></v8:NumberQualifiers></Type>'
        '</Properties></Attribute></ChildObjects></ExchangePlan></MetaDataObject>') % (
            PLAN_UUID, PLAN, ''.join(std(n, 'ShowError' if n in ('Description', 'Code') else 'DontCheck')
                                     for n in ('ThisNode', 'ReceivedNo', 'SentNo', 'Ref', 'DeletionMark',
                                               'Description', 'Code'))),
    'Constants/КонстантаУзел.xml': HEAD + (
        '<Constant uuid="7a8b9c0d-1e2f-4a3b-8c4d-5e6f7a8b9c0d">' + CONST_INFO + '<Properties><Name>КонстантаУзел</Name><Synonym/>'
        '<Comment/><Type><v8:Type>%s</v8:Type></Type></Properties></Constant></MetaDataObject>') % REF,
    'SessionParameters/ПараметрУзел.xml': HEAD + (
        '<SessionParameter uuid="1b2c3d4e-5f60-4718-9a2b-3c4d5e6f7081"><Properties><Name>ПараметрУзел</Name><Synonym/>'
        '<Comment/><Type><v8:Type>%s</v8:Type></Type></Properties></SessionParameter></MetaDataObject>') % REF,
    'DefinedTypes/ТипУзла.xml': HEAD + (
        '<DefinedType uuid="2c3d4e5f-6071-4829-8a3b-4c5d6e7f8091">' + DEFINED_INFO + '<Properties><Name>ТипУзла</Name><Synonym/>'
        '<Comment/><Type><v8:Type>%s</v8:Type></Type></Properties></DefinedType></MetaDataObject>') % REF,
}
CHILDREN = ('<Language>Русский</Language><SessionParameter>ПараметрУзел</SessionParameter>'
            '<ExchangePlan>%s</ExchangePlan><DefinedType>ТипУзла</DefinedType>'
            '<Constant>КонстантаУзел</Constant>') % PLAN


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
                fields.append(text[cur:i])
                return fields, i + 1
            depth -= 1
        elif ch == ',' and depth == 0:
            fields.append(text[cur:i])
            cur = i + 1
        i += 1
    raise ValueError('unterminated list')


def join(fields):
    return '{' + ','.join(fields) + '}'


STD_KEY = '3b10624f-1e3d-495d-8093-25225efc5313'


def downgrade_bag(bag):
    """`{14,25,…}` standard attribute bag -> `{13,24,…}`."""
    f, _ = split_fields(bag, 0)
    assert f[0].strip() == '14' and f[1].strip() == '25' and len(f) == 52, f[:2]
    assert f[12].strip() == STD_KEY, f[12]
    return join(['13', '24'] + f[2:12] + f[14:])


def downgrade_std(field):
    outer, _ = split_fields(field.strip(), 0)
    assert outer[0].strip() == '1', outer[0]
    payload, _ = split_fields(outer[1].strip(), 0)
    assert payload[0].strip() == '1' and payload[1].strip() == '7', payload[:2]
    for k in range(2, len(payload), 3):
        payload[k + 2] = downgrade_bag(payload[k + 2].strip())
    return join([outer[0], join(payload)])


def downgrade_attribute(item):
    """`{{4,<common>,i,f,d,0,{1,<nil>}},0}` -> `{{3,<common>,i,f,d},0}`."""
    it, _ = split_fields(item.strip(), 0)
    w, _ = split_fields(it[0].strip(), 0)
    assert w[0].strip() == '4' and len(w) == 7 and w[5].strip() == '0', w[0]
    assert re.fullmatch(r'\{1,%s\}' % NIL, re.sub(r'\s', '', w[6])), w[6]
    return join([join(['3'] + w[1:5])] + it[1:])


def downgrade(text, version):
    body = text.lstrip('\ufeff')
    root, end = split_fields(body, body.index('{'))
    owner, _ = split_fields(root[1].strip(), 0)
    assert owner[0].strip() == '37' and len(owner) == 51 and owner[49].strip() == '0' and owner[50].strip() == '1'
    owner[30] = downgrade_std(owner[30])
    owner = [version] + owner[1:(49 if version == '35' else 50)]
    root[1] = join(owner)
    attrs, _ = split_fields(root[3].strip(), 0)   # {<class>,<count>,items…}
    attrs = attrs[:2] + [downgrade_attribute(a) for a in attrs[2:]]
    root[3] = join(attrs)
    return '\ufeff' + join(root) + body[end:]


def platform_dump(cf, work, name):
    ib = os.path.join(work, 'ib-' + name); log = os.path.join(work, name + '.log')
    out = os.path.join(work, 'dump-' + name)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadCfg', cf], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', out], log)
    return out


def tree(root):
    out = {}
    for dp, _, fs in os.walk(root):
        for f in fs:
            p = os.path.join(dp, f)
            out[os.path.relpath(p, root).replace(os.sep, '/')] = open(p, 'rb').read()
    return out


HEADER = re.compile(r'\{3,\s*\{1,0,([0-9a-f]{8}-[0-9a-f-]{27})\},')


def old_headers(text, root_uuid=None, style='one'):
    """Every header block `{3,{1,0,u},N,S,C,0,0,<nil>,0}` in the older
    spelling `{1,{0,0,u},N,S,C,0,0}`; in the root record also the sections'
    object identities `{1,0,id}` -> `{0,0,id}` (a root saved in 8.3.9 mode)."""
    out, done, n = [], 0, 0
    for m in HEADER.finditer(text):
        if m.start() < done:
            continue
        fields, end = split_fields(text, m.start())
        tail = [f.strip() for f in fields[5:]]
        if len(fields) != 9 or tail != ['0', '0', NIL, '0']:
            continue
        out.append(text[done:m.start()])
        spelled = (fields[2].strip(), fields[3].strip(), fields[4].strip())
        if style == 'one':
            out.append('{1,{0,0,%s},%s,%s,%s,0,0}' % ((m.group(1),) + spelled))
        else:
            # `{2,{1,0,u},N,S,C,0,0,<nil>}` (a configuration saved in 8.3.14 mode)
            out.append('{2,{1,0,%s},%s,%s,%s,0,0,%s}' % ((m.group(1),) + spelled + (NIL,)))
        done, n = end, n + 1
    out.append(text[done:])
    text = ''.join(out)
    if root_uuid and style == 'one':
        text = re.sub(r'\{1,0,([0-9a-f]{8}-[0-9a-f-]{27})\}', r'{0,0,\1}', text)
    return text, n


def derive_old_headers(work, style, out_name):
    """`old_headers.cf`: current.cf with every header block (and the root's
    section identities) in the spelling of a configuration saved in 8.3.9
    mode; the platform must dump it to the expected XML."""
    current = os.path.join(DST, 'current.cf')
    raw = open(current, 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(current)
    root_uuid = re.search(r'[0-9a-f]{8}-[0-9a-f-]{27}', entries['root'].decode('utf-8-sig')).group(0)
    total = 0
    for name, body in list(entries.items()):
        if len(name) != 36:
            continue
        try:
            text = body.decode('utf-8-sig')
        except UnicodeDecodeError:
            continue
        new, n = old_headers(text, root_uuid if name == root_uuid else None, style)
        if new != text:
            entries[name] = ('﻿' + new).encode('utf-8')
            total += n
    path = os.path.join(DST, out_name)
    open(path, 'wb').write(v8c.write15(entries, storage_version))
    base = tree(os.path.join(DST, 'expected'))
    got = tree(platform_dump(path, work, out_name[:-3]))
    diff = sorted(k for k in set(base) | set(got) if base.get(k) != got.get(k) and k != 'ConfigDumpInfo.xml')
    # IntegrationService's ObjectId is drawn afresh on every load.
    assert diff == ['Configuration.xml'] or not diff, diff
    if diff:
        a = base['Configuration.xml'].decode('utf-8-sig').splitlines()
        b = got['Configuration.xml'].decode('utf-8-sig').splitlines()
        changed = [(x, y) for x, y in zip(a, b) if x != y]
        assert len(a) == len(b) and len(changed) == 1 and '<xr:ObjectId>' in changed[0][0], changed
    print('ok', out_name, total, 'blocks; platform dump equals the expected dump')


CONSTANT_UUID = '7a8b9c0d-1e2f-4a3b-8c4d-5e6f7a8b9c0d'


def derive_constant_v14(work):
    """`constant_v14.cf`: the constant stored as `{14,…}` of 12 members (the
    first 12 of its `{16,…}`: no ValueKey type ids), as a configuration in
    8.3.9 mode keeps it. Prints whether the platform's dump of it names the
    same ValueKey ids as the original (it derives them on load)."""
    current = os.path.join(DST, 'current.cf')
    raw = open(current, 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(current)
    text = entries[CONSTANT_UUID].decode('utf-8-sig')
    root, end = split_fields(text, text.index('{'))
    owner, _ = split_fields(root[1].strip(), 0)
    assert owner[0].strip() == '16' and len(owner) == 17, (owner[0], len(owner))
    root[1] = join(['14'] + owner[1:12])
    entries[CONSTANT_UUID] = ('﻿' + join(root) + text[end:]).encode('utf-8')
    path = os.path.join(DST, 'constant_v14.cf')
    open(path, 'wb').write(v8c.write15(entries, storage_version))
    base = tree(os.path.join(DST, 'expected'))
    got = tree(platform_dump(path, work, 'constant-v14'))
    rel = 'Constants/КонстантаУзел.xml'
    print('constant_v14.cf: platform dump of the constant equals the original:', base[rel] == got[rel])
    a, b = base[rel].decode('utf-8-sig').splitlines(), got[rel].decode('utf-8-sig').splitlines()
    for x, y in zip(a, b):
        if x != y:
            print('   original', x.strip(), '\n   derived ', y.strip())


def main():
    work = tempfile.mkdtemp(prefix='upgrade-fixture-')
    if '--constant-v14' in sys.argv:
        derive_constant_v14(work)
        shutil.rmtree(work, ignore_errors=True)
        return
    if '--old-headers' in sys.argv:
        derive_old_headers(work, 'one', 'old_headers.cf')
        derive_old_headers(work, 'two', 'old_headers_2.cf')
        shutil.rmtree(work, ignore_errors=True)
        return
    src = os.path.join(work, 'src'); shutil.copytree(SRC, src)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<ChildObjects>\n\t\t\t<Language>Русский</Language>\n\t\t</ChildObjects>',
                  '<ChildObjects>%s</ChildObjects>' % CHILDREN)
    assert CHILDREN in t
    # The corpora's plans come from configurations in 8.3.17/8.3.21 mode: an
    # 8.3.27-mode plan also carries ExchangeDate (eight standard attributes).
    t = t.replace('<CompatibilityMode>Version8_3_27<', '<CompatibilityMode>Version8_3_21<')
    t = t.replace('<ConfigurationExtensionCompatibilityMode>Version8_3_27<',
                  '<ConfigurationExtensionCompatibilityMode>Version8_3_21<')
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    for rel, xml in OBJECTS.items():
        p = os.path.join(src, rel); os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, 'w', encoding='utf-8-sig').write(xml)
    shutil.rmtree(DST, ignore_errors=True); os.makedirs(DST)
    current = os.path.join(DST, 'current.cf')
    ib = os.path.join(work, 'ib'); log = os.path.join(work, 'load.log')
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', current], log)
    expected = os.path.join(DST, 'expected')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', expected], log)
    base = tree(expected)

    raw = open(current, 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(current)
    text = entries[PLAN_UUID].decode('utf-8-sig')
    if '--explore' in sys.argv:
        print(text[:3000])
    for version in ('35', '36'):
        derived = dict(entries)
        derived[PLAN_UUID] = downgrade(text, version).encode('utf-8')
        path = os.path.join(DST, 'v%s.cf' % version)
        open(path, 'wb').write(v8c.write15(derived, storage_version))
        got = tree(platform_dump(path, work, 'v' + version))
        diff = sorted(k for k in set(base) | set(got) if base.get(k) != got.get(k) and k != 'ConfigDumpInfo.xml')
        assert not diff, (version, diff)
        print('ok', version, 'platform dump of the derived .cf equals the expected dump')
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
