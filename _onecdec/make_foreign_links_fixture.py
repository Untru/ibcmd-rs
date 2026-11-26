"""Build the foreign-links fixture with the platform: an extension with two
documents of its own; an attribute of the second links its choice parameters
and its type to an attribute of the first document's tabular section (two
data-path segments, both foreign to the linking document).

    python make_foreign_links_fixture.py

Output: tests/fixtures/external/adopted/foreign_links/{input.cfe, Documents/*.xml}
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted
import make_adopted_properties_fixture as props

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT, write, md = adopted.EXE, adopted.EXT, adopted.write, props.md
P = 'ТестРасширение_'


def uid(n):
    return 'e5000000-0000-4000-8000-%012d' % n


def gen(items, seed):
    return '\n'.join(
        '%s<xr:GeneratedType name="%s" category="%s">\n%s\t<xr:TypeId>%s-0000-4000-8000-%012d</xr:TypeId>\n'
        '%s\t<xr:ValueId>%s-0000-4000-8000-%012d</xr:ValueId>\n%s</xr:GeneratedType>'
        % (ind, n, c, ind, seed, 2 * i + 1, ind, seed, 2 * i + 2, ind) for i, (n, c, ind) in enumerate(items))


STRING = '<Type>\n{i}\t<v8:Type>xs:string</v8:Type>\n{i}\t<v8:StringQualifiers>\n{i}\t\t<v8:Length>10</v8:Length>\n{i}\t\t<v8:AllowedLength>Variable</v8:AllowedLength>\n{i}\t</v8:StringQualifiers>\n{i}</Type>'


def attribute(uuid, name, indent, extra=''):
    i = indent
    return ('{i}<Attribute uuid="{u}">\n{i}\t<Properties>\n{i}\t\t<Name>{n}</Name>\n{i}\t\t<Synonym/>\n{i}\t\t<Comment/>\n'
            '{i}\t\t{t}\n{e}{i}\t</Properties>\n{i}</Attribute>\n').format(i=i, u=uuid, n=name, t=STRING.format(i=i + '\t\t'), e=extra)


def document(uuid, name, seed, children):
    cats = [('Document%s.%s' % (a, name), a, '\t\t\t') for a in ['Object', 'Ref', 'Selection', 'List', 'Manager']]
    return md('\t<Document uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<Name>%s</Name>\n'
              '\t\t\t<Synonym/>\n\t\t\t<Comment/>\n\t\t</Properties>\n\t\t<ChildObjects>\n%s\t\t</ChildObjects>\n\t</Document>'
              % (uuid, gen(cats, seed), name, children))


def main():
    work = tempfile.mkdtemp(prefix='foreign-links-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    a, b = P + 'Источник', P + 'Приемник'
    ts = ('\t\t\t<TabularSection uuid="%s">\n\t\t\t\t<InternalInfo>\n%s\n\t\t\t\t</InternalInfo>\n\t\t\t\t<Properties>\n'
          '\t\t\t\t\t<Name>Строки</Name>\n\t\t\t\t\t<Synonym/>\n\t\t\t\t\t<Comment/>\n\t\t\t\t</Properties>\n\t\t\t\t<ChildObjects>\n%s'
          '\t\t\t\t</ChildObjects>\n\t\t\t</TabularSection>\n'
          % (uid(2), gen([('DocumentTabularSection.%s.Строки' % a, 'TabularSection', '\t\t\t\t\t'),
                          ('DocumentTabularSectionRow.%s.Строки' % a, 'TabularSectionRow', '\t\t\t\t\t')], 'd5000002'),
             attribute(uid(3), 'Поле', '\t\t\t\t\t')))
    path = '0:%s/0:%s' % (uid(2), uid(3))
    links = ('\t\t\t\t\t<ChoiceParameterLinks>\n\t\t\t\t\t\t<xr:Link>\n\t\t\t\t\t\t\t<xr:Name>Отбор.Код</xr:Name>\n'
             '\t\t\t\t\t\t\t<xr:DataPath xsi:type="xs:string">%s</xr:DataPath>\n\t\t\t\t\t\t\t<xr:ValueChange>Clear</xr:ValueChange>\n'
             '\t\t\t\t\t\t</xr:Link>\n\t\t\t\t\t</ChoiceParameterLinks>\n'
             '\t\t\t\t\t<LinkByType>\n\t\t\t\t\t\t<xr:DataPath>%s</xr:DataPath>\n\t\t\t\t\t\t<xr:LinkItem>0</xr:LinkItem>\n'
             '\t\t\t\t\t</LinkByType>\n' % (path, path))
    write(os.path.join(ext, 'Documents', a + '.xml'), document(uid(1), a, 'd5000001', ts))
    write(os.path.join(ext, 'Documents', b + '.xml'), document(uid(4), b, 'd5000004', attribute(uid(5), 'Связь', '\t\t\t', links)))
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                       [('Document', a), ('Document', b)])
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'foreign_links')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    os.makedirs(os.path.join(dst, 'Documents'))
    for n in (a, b):
        shutil.copy(os.path.join(dump, 'Documents', n + '.xml'), os.path.join(dst, 'Documents'))
    t = open(os.path.join(dump, 'Documents', b + '.xml'), encoding='utf-8-sig').read()
    print(t[t.find('<ChoiceParameterLinks>'):t.find('</LinkByType>') + 14])
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
