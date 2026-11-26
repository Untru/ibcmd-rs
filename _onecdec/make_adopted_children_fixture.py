"""Build the adopted-children fixture with the platform: a base configuration
with one document (an attribute; a tabular section with an attribute) and an
extension adopting the document, its tabular section and both attributes --
one attribute with its type controlled -- with the object module extended.
The shape a real extension keeps for 65 adopted catalogs and documents.

    python make_adopted_children_fixture.py

Output: tests/fixtures/external/adopted/document_children/{input.cfe, Documents/Документ.xml}
"""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE = adopted.EXE
EXT = adopted.EXT
md, write = adopted.md, adopted.write

BASE_DOC, BASE_ATTR, BASE_TS, BASE_TS_ATTR = ('b1000000-0000-4000-8000-00000000000%d' % i for i in range(1, 5))
EXT_DOC, EXT_ATTR, EXT_TS, EXT_TS_ATTR = ('e1000000-0000-4000-8000-00000000000%d' % i for i in range(1, 5))

STRING = ('<Type>\n\t\t\t\t\t\t<v8:Type>xs:string</v8:Type>\n\t\t\t\t\t\t<v8:StringQualifiers>\n'
          '\t\t\t\t\t\t\t<v8:Length>10</v8:Length>\n\t\t\t\t\t\t\t<v8:AllowedLength>Variable</v8:AllowedLength>\n'
          '\t\t\t\t\t\t</v8:StringQualifiers>\n\t\t\t\t\t</Type>')
NUMBER = ('<Type>\n\t\t\t\t\t\t\t\t<v8:Type>xs:decimal</v8:Type>\n\t\t\t\t\t\t\t\t<v8:NumberQualifiers>\n'
          '\t\t\t\t\t\t\t\t\t<v8:Digits>10</v8:Digits>\n\t\t\t\t\t\t\t\t\t<v8:FractionDigits>0</v8:FractionDigits>\n'
          '\t\t\t\t\t\t\t\t\t<v8:AllowedSign>Any</v8:AllowedSign>\n\t\t\t\t\t\t\t\t</v8:NumberQualifiers>\n\t\t\t\t\t\t\t</Type>')


def generated_ts(doc, ts, seed):
    out = []
    for i, (kind, cat) in enumerate([('DocumentTabularSection', 'TabularSection'), ('DocumentTabularSectionRow', 'TabularSectionRow')]):
        out.append('\t\t\t\t\t<xr:GeneratedType name="%s.%s.%s" category="%s">\n\t\t\t\t\t\t<xr:TypeId>%s-0000-4000-8000-%012d</xr:TypeId>\n'
                   '\t\t\t\t\t\t<xr:ValueId>%s-0000-4000-8000-%012d</xr:ValueId>\n\t\t\t\t\t</xr:GeneratedType>'
                   % (kind, doc, ts, cat, seed, 2 * i + 1, seed, 2 * i + 2))
    return '\n'.join(out)


def document(uuid, seed, adopted_from=None, children=None):
    belonging = ('\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>Документ</Name>\n\t\t\t<Comment/>\n'
                 '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % adopted_from
                 if adopted_from else '\t\t\t<Name>Документ</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n')
    states = adopted.states(['ObjectModule']) if adopted_from else ''
    return md('\t<Document uuid="%s">\n\t\t<InternalInfo>\n%s%s\n\t\t</InternalInfo>\n\t\t<Properties>\n%s\t\t</Properties>\n'
              '\t\t<ChildObjects>\n%s\n\t\t</ChildObjects>\n\t</Document>'
              % (uuid, adopted.generated('Document', 'Документ', seed), states, belonging, children))


def attribute(uuid, name, type_xml, adopted_from=None, indent='\t\t\t'):
    if adopted_from:
        props = ('<ObjectBelonging>Adopted</ObjectBelonging>\n{i}\t\t<Name>{n}</Name>\n{i}\t\t<Comment/>\n'
                 '{i}\t\t<ExtendedConfigurationObject>{a}</ExtendedConfigurationObject>\n').format(i=indent, n=name, a=adopted_from)
        props = '{i}\t\t'.format(i=indent) + props + ('{i}\t\t{t}\n'.format(i=indent, t=type_xml) if type_xml else '')
        return '{i}<Attribute uuid="{u}">\n{i}\t<InternalInfo/>\n{i}\t<Properties>\n{p}{i}\t</Properties>\n{i}</Attribute>'.format(
            i=indent, u=uuid, p=props)
    return ('{i}<Attribute uuid="{u}">\n{i}\t<Properties>\n{i}\t\t<Name>{n}</Name>\n{i}\t\t<Synonym/>\n{i}\t\t<Comment/>\n'
            '{i}\t\t{t}\n{i}\t</Properties>\n{i}</Attribute>').format(i=indent, u=uuid, n=name, t=type_xml)


def tabular(uuid, seed, attr, adopted_from=None):
    props = ('\t\t\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t\t\t<Name>Товары</Name>\n\t\t\t\t\t<Comment/>\n'
             '\t\t\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % adopted_from
             if adopted_from else '\t\t\t\t\t<Name>Товары</Name>\n\t\t\t\t\t<Synonym/>\n\t\t\t\t\t<Comment/>\n')
    return ('\t\t\t<TabularSection uuid="%s">\n\t\t\t\t<InternalInfo>\n%s\n\t\t\t\t</InternalInfo>\n\t\t\t\t<Properties>\n%s'
            '\t\t\t\t</Properties>\n\t\t\t\t<ChildObjects>\n%s\n\t\t\t\t</ChildObjects>\n\t\t\t</TabularSection>'
            % (uuid, generated_ts('Документ', 'Товары', seed), props, attr))


def main():
    work = os.path.join(HERE, '.fixture-work-adopted-children'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    write(os.path.join(base, 'Documents', 'Документ.xml'), document(BASE_DOC, 'd0000001', children='\n'.join([
        attribute(BASE_ATTR, 'Реквизит', STRING),
        tabular(BASE_TS, 'd0000002', attribute(BASE_TS_ATTR, 'Количество', NUMBER, indent='\t\t\t\t\t'))])))
    write(os.path.join(base, 'Documents', 'Документ', 'Ext', 'ObjectModule.bsl'), 'Процедура ПередЗаписью(Отказ)\nКонецПроцедуры\n', True)
    cfg = os.path.join(base, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    open(cfg, 'w', encoding='utf-8-sig').write(t.replace('<Language>Русский</Language>', '<Language>Русский</Language>\n\t\t\t<Document>Документ</Document>', 1))

    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    write(os.path.join(ext, 'Documents', 'Документ.xml'), document(EXT_DOC, 'd0000003', BASE_DOC, '\n'.join([
        attribute(EXT_ATTR, 'Реквизит', None, BASE_ATTR),
        tabular(EXT_TS, 'd0000004', attribute(EXT_TS_ATTR, 'Количество', NUMBER, BASE_TS_ATTR, indent='\t\t\t\t\t'), BASE_TS)])))
    write(os.path.join(ext, 'Documents', 'Документ', 'Ext', 'ObjectModule.bsl'), '// расширение модуля объекта\n', True)
    cfg = os.path.join(ext, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    anchor = '<CommonModule>ТестРасширение_Модуль</CommonModule>'
    assert anchor in t
    open(cfg, 'w', encoding='utf-8-sig').write(t.replace(anchor, anchor + '\n\t\t\t<Document>Документ</Document>', 1))

    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'document_children')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    os.makedirs(os.path.join(dst, 'Documents'))
    shutil.copy(os.path.join(dump, 'Documents', 'Документ.xml'), os.path.join(dst, 'Documents'))
    print('ok', open(os.path.join(dst, 'Documents', 'Документ.xml'), encoding='utf-8-sig').read().count('\n'), 'lines')
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
