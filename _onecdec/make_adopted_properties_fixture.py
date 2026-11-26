"""Build adopted-property fixtures with the platform: a base configuration with
one object of several kinds, and an extension adopting each of them with a
chosen subset of their properties controlled. Every case keeps the .cfe and
the platform's own dump of the adopted objects.

    python make_adopted_properties_fixture.py [case ...]   # default: every case

Output: tests/fixtures/external/adopted/props_<case>/{input.cfe, <dumped xml>}
Prints each adopted row's header pairs beside the properties the dump
prints: `all` controls every property, `b0`..`b2` only those whose index in
the object's list has that bit set -- which sets each property uuid apart.
"""
import os, re, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump, v8c
import make_adopted_fixtures as adopted

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT, write = adopted.EXE, adopted.EXT, adopted.write
NS = adopted.NS.replace('xmlns:v8=', 'xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" xmlns:v8=')


def md(body):
    return '﻿<?xml version="1.0" encoding="UTF-8"?>\n<MetaDataObject %s>\n%s\n</MetaDataObject>' % (NS, body)


def uid(side, n):
    return '%s3000000-0000-4000-8000-%012d' % (side, n)


def generated(kind, name, seed, cats):
    out = []
    for i, (prefix, cat) in enumerate(cats):
        out.append('\t\t\t<xr:GeneratedType name="%s.%s" category="%s">\n\t\t\t\t<xr:TypeId>%s-0000-4000-8000-%012d</xr:TypeId>\n'
                   '\t\t\t\t<xr:ValueId>%s-0000-4000-8000-%012d</xr:ValueId>\n\t\t\t</xr:GeneratedType>'
                   % (prefix, name, cat, seed, 2 * i + 1, seed, 2 * i + 2))
    return '\n'.join(out)


OBJ = [('Object', 'Object'), ('Ref', 'Ref'), ('Selection', 'Selection'), ('List', 'List'), ('Manager', 'Manager')]
CATS = {
    'Catalog': [('Catalog' + a, b) for a, b in OBJ],
    'Document': [('Document' + a, b) for a, b in OBJ],
    'Enum': [('EnumRef', 'Ref'), ('EnumManager', 'Manager'), ('EnumList', 'List')],
    'InformationRegister': [('InformationRegister' + a, a) for a in
                            ['Record', 'Manager', 'Selection', 'List', 'RecordSet', 'RecordKey', 'RecordManager']],
    'AccumulationRegister': [('AccumulationRegister' + a, a) for a in
                             ['Record', 'Manager', 'Selection', 'List', 'RecordSet', 'RecordKey']],
}
FOLDER = {'Catalog': 'Catalogs', 'Document': 'Documents', 'Enum': 'Enums', 'InformationRegister': 'InformationRegisters',
          'AccumulationRegister': 'AccumulationRegisters', 'Subsystem': 'Subsystems', 'StyleItem': 'StyleItems',
          'CommonCommand': 'CommonCommands'}
HAS_CHILDREN = {'Catalog', 'Document', 'Enum', 'InformationRegister', 'AccumulationRegister', 'Subsystem'}

SYNONYM = '<Synonym>\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>ru</v8:lang>\n\t\t\t\t\t<v8:content>Синоним</v8:content>\n\t\t\t\t</v8:item>\n\t\t\t</Synonym>'

# kind, name, seed, properties (element lines, each controllable), extras in
# the base only, base children, extension children (per case: `ext_children`)
OBJECTS = [
    ('Catalog', 'Владелец', 1, [], [], '', ''),
    ('Catalog', 'Справочник', 2, [
        '<Owners>\n\t\t\t\t<xr:Item xsi:type="xr:MDObjectRef">Catalog.Владелец</xr:Item>\n\t\t\t</Owners>',
        '<Hierarchical>true</Hierarchical>',
        '<HierarchyType>HierarchyOfItems</HierarchyType>',
        '<CodeLength>11</CodeLength>',
        '<DescriptionLength>50</DescriptionLength>',
        '<CodeType>Number</CodeType>',
        '<CodeAllowedLength>Fixed</CodeAllowedLength>',
    ], [], '', ''),
    ('Document', 'Документ', 3, [
        SYNONYM,
        '<NumberType>String</NumberType>',
        '<NumberLength>11</NumberLength>',
        '<NumberAllowedLength>Fixed</NumberAllowedLength>',
    ], [], '', ''),
    ('InformationRegister', 'РегистрСведений', 4, [
        '<InformationRegisterPeriodicity>Day</InformationRegisterPeriodicity>',
        '<WriteMode>Independent</WriteMode>',
    ], [], '', ''),
    ('AccumulationRegister', 'РегистрНакопления', 5, [
        '<RegisterType>Turnovers</RegisterType>',
    ], [], '', ''),
    ('Enum', 'Перечисление', 6, [], [], 'values', 'values'),
    ('Subsystem', 'Подсистема', 7, [
        '<Content>\n\t\t\t\t<xr:Item xsi:type="xr:MDObjectRef">Catalog.Справочник</xr:Item>\n\t\t\t</Content>',
    ], [], '', ''),
    ('StyleItem', 'ЭлементСтиля', 8, [
        '<Type>Color</Type>',
        '<Value xsi:type="v8ui:Color">web:Gray</Value>',
    ], [], '', ''),
    ('CommonCommand', 'ОбщаяКоманда', 9, [
        '<Group>FormNavigationPanelGoTo</Group>',
    ], [], '', ''),
]
ENUM_VALUES = ['Первое', 'Второе']


def resource(uuid, tag, name, type_xml):
    return ('\t\t\t<%s uuid="%s">\n\t\t\t\t<Properties>\n\t\t\t\t\t<Name>%s</Name>\n\t\t\t\t\t<Synonym/>\n\t\t\t\t\t<Comment/>\n'
            '\t\t\t\t\t<Type>\n\t\t\t\t\t\t%s\n\t\t\t\t\t</Type>\n\t\t\t\t</Properties>\n\t\t\t</%s>' % (tag, uuid, name, type_xml, tag))


NUMBER10 = ('<v8:Type>xs:decimal</v8:Type>\n\t\t\t\t\t\t<v8:NumberQualifiers>\n\t\t\t\t\t\t\t<v8:Digits>10</v8:Digits>\n'
            '\t\t\t\t\t\t\t<v8:FractionDigits>0</v8:FractionDigits>\n\t\t\t\t\t\t\t<v8:AllowedSign>Any</v8:AllowedSign>\n'
            '\t\t\t\t\t\t</v8:NumberQualifiers>')
# Only in the base: what makes it load.
BASE_EXTRA = {
    'Документ': ['<RegisterRecords>\n\t\t\t\t<xr:Item xsi:type="xr:MDObjectRef">AccumulationRegister.РегистрНакопления</xr:Item>\n\t\t\t</RegisterRecords>'],
}
BASE_CHILDREN = {
    'РегистрСведений': resource(uid('b', 200), 'Resource', 'Ресурс', NUMBER10),
    'РегистрНакопления': resource(uid('b', 201), 'Resource', 'Количество', NUMBER10),
}

# The root: controlled properties inserted before / after InterfaceCompatibilityMode.
ROOT_PROPS = [('before', '<ModalityUseMode>UseWithWarnings</ModalityUseMode>'),
              ('after', '<CompatibilityMode>Version8_3_27</CompatibilityMode>')]


def enum_values(side, adopt):
    out = []
    for i, name in enumerate(ENUM_VALUES):
        if adopt:
            props = ('\t\t\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t\t\t<Name>%s</Name>\n\t\t\t\t\t<Comment/>\n'
                     '\t\t\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % (name, uid('b', 100 + i)))
            if i == 1:  # only the first value is adopted
                continue
            out.append('\t\t\t<EnumValue uuid="%s">\n\t\t\t\t<InternalInfo/>\n\t\t\t\t<Properties>\n%s\t\t\t\t</Properties>\n\t\t\t</EnumValue>'
                       % (uid('e', 100 + i), props))
        else:
            out.append('\t\t\t<EnumValue uuid="%s">\n\t\t\t\t<Properties>\n\t\t\t\t\t<Name>%s</Name>\n\t\t\t\t\t<Synonym/>\n\t\t\t\t\t<Comment/>\n'
                       '\t\t\t\t</Properties>\n\t\t\t</EnumValue>' % (uid('b', 100 + i), name))
    return '\n'.join(out)


def object_xml(kind, name, seed, props, side, adopt_from=None):
    cats = CATS.get(kind)
    info = '\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n' % generated(kind, name, '%s%07d' % ('c' if side == 'b' else 'd', seed), cats) if cats else ''
    if adopt_from:
        head = ('\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>%s</Name>\n\t\t\t<Comment/>\n'
                '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % (name, adopt_from))
        if not cats:
            info = '\t\t<InternalInfo/>\n'
    else:
        head = '\t\t\t<Name>%s</Name>\n' % name
        if not any(p.startswith('<Synonym>') for p in props):
            head += '\t\t\t<Synonym/>\n'
        head += '\t\t\t<Comment/>\n'
    lines = ''
    for p in props:
        if p.startswith('<Synonym>') and not adopt_from:
            head = head.replace('\t\t\t<Name>%s</Name>\n' % name, '\t\t\t<Name>%s</Name>\n\t\t\t%s\n' % (name, p))
            continue
        lines += '\t\t\t%s\n' % p
    if p_synonym_first(props) and adopt_from:
        # an adopted Synonym sits after the name, before the comment
        syn = [p for p in props if p.startswith('<Synonym>')][0]
        head = head.replace('\t\t\t<Comment/>\n', '\t\t\t%s\n\t\t\t<Comment/>\n' % syn)
        lines = lines.replace('\t\t\t%s\n' % syn, '')
    if not adopt_from:
        lines += ''.join('\t\t\t%s\n' % p for p in BASE_EXTRA.get(name, []))
    children = ''
    if kind in HAS_CHILDREN:
        body = enum_values(side, adopt_from is not None) if kind == 'Enum' else ''
        if not adopt_from:
            body = body or BASE_CHILDREN.get(name, '')
        children = '\t\t<ChildObjects>\n%s\n\t\t</ChildObjects>\n' % body if body else '\t\t<ChildObjects/>\n'
    uuid = uid(side, seed)
    return md('\t<%s uuid="%s">\n%s\t\t<Properties>\n%s%s\t\t</Properties>\n%s\t</%s>' % (kind, uuid, info, head, lines, children, kind))


def p_synonym_first(props):
    return any(p.startswith('<Synonym>') for p in props)


def chosen(props, case):
    if case == 'all':
        return props
    bit = int(case[1:])
    return [p for i, p in enumerate(props) if i & (1 << bit)]


def add_children(cfg, anchor, names):
    t = open(cfg, encoding='utf-8-sig').read()
    assert anchor in t
    t = t.replace(anchor, anchor + ''.join('\n\t\t\t<%s>%s</%s>' % (k, n, k) for k, n in names), 1)
    open(cfg, 'w', encoding='utf-8-sig').write(t)


def sources(work, case):
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    names = []
    for kind, name, seed, props, _, _, _ in OBJECTS:
        write(os.path.join(base, FOLDER[kind], name + '.xml'), object_xml(kind, name, seed, props, 'b'))
        write(os.path.join(ext, FOLDER[kind], name + '.xml'), object_xml(kind, name, seed, chosen(props, case), 'e', uid('b', seed)))
        if kind == 'CommonCommand':
            write(os.path.join(base, FOLDER[kind], name, 'Ext', 'CommandModule.bsl'),
                  '&НаКлиенте\nПроцедура ОбработкаКоманды(ПараметрКоманды, ПараметрыВыполненияКоманды)\nКонецПроцедуры\n', True)
        names.append((kind, name))
    add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>', names)
    add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>', names)
    cfg = os.path.join(ext, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    icm = '\t\t\t<InterfaceCompatibilityMode>TaxiEnableVersion8_2</InterfaceCompatibilityMode>\n'
    assert icm in t
    for where, p in chosen(ROOT_PROPS, case):
        t = t.replace(icm, '\t\t\t%s\n%s' % (p, icm) if where == 'before' else '%s\t\t\t%s\n' % (icm, p), 1)
        icm = icm if where == 'after' else icm
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    return base, ext


HDR = re.compile(r'\{[23],\s*\{1,0,([0-9a-f-]{36})\},"([^"]*)",\s*\{[^{}]*\},"[^"]*",1,(\d+),((?:\s*[0-9a-f-]{36},\d+,)*)')


def build(case, dst_root):
    work = tempfile.mkdtemp(prefix='adopted-props-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base, ext = sources(work, case)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(dst_root, 'props_' + case); shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    cfe = os.path.join(dst, 'input.cfe')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', cfe, '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    rels = ['Configuration.xml'] + ['%s/%s.xml' % (FOLDER[k], n) for k, n, *_ in OBJECTS]
    for rel in rels:
        src = os.path.join(dump, rel)
        os.makedirs(os.path.dirname(os.path.join(dst, rel)), exist_ok=True)
        shutil.copy(src, os.path.join(dst, rel))
    # the header pairs beside what the dump prints
    rows = v8c.read(cfe)
    for name, raw in rows.items():
        if len(name) != 36:
            continue
        text = raw.decode('utf-8-sig', 'replace')
        for m in HDR.finditer(text):
            pairs = re.findall(r'([0-9a-f-]{36}),(\d)', m.group(4))
            print(case, m.group(2), pairs)
    for rel in rels:
        t = open(os.path.join(dump, rel), encoding='utf-8-sig').read()
        body = t.split('<Properties>')[1].split('</Properties>')[0]
        print(case, rel, re.findall(r'^\t\t\t<(\w+)', body, flags=re.M), re.findall(r'<xr:Property>(\w+)</xr:Property>', t))
    shutil.rmtree(work, ignore_errors=True)


CASES = ['all', 'b0', 'b1', 'b2']


def main():
    names = sys.argv[1:] or CASES
    dst_root = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted')
    for name in names:
        build(name, dst_root)


if __name__ == '__main__':
    main()
