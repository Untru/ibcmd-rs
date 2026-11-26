"""Build the widened-type fixture with the platform: a base configuration with
two defined types and a catalog (an attribute of any reference type), and an
extension that adopts them -- one defined type widened by a catalog of the
extension, one adopted as is, the attribute's type widened -- and a filter
criterion whose type is widened.

    python make_adopted_widened_fixture.py

Output: tests/fixtures/external/adopted/widened/{input.cfe, <dumped xml>}
"""
import os, re, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump, v8c
import make_adopted_fixtures as adopted
import make_adopted_properties_fixture as props

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT, write, md = adopted.EXE, adopted.EXT, adopted.write, props.md


def uid(side, n):
    return '%s4000000-0000-4000-8000-%012d' % (side, n)


def gen(name_cat, seed):
    return props.generated('', '', seed, []) if False else '\n'.join(
        '\t\t\t<xr:GeneratedType name="%s" category="%s">\n\t\t\t\t<xr:TypeId>%s-0000-4000-8000-%012d</xr:TypeId>\n'
        '\t\t\t\t<xr:ValueId>%s-0000-4000-8000-%012d</xr:ValueId>\n\t\t\t</xr:GeneratedType>'
        % (n, c, seed, 2 * i + 1, seed, 2 * i + 2) for i, (n, c) in enumerate(name_cat))


def catalog_cats(name):
    return [('Catalog%s.%s' % (a, name), a) for a in ['Object', 'Ref', 'Selection', 'List', 'Manager']]


def types(refs, indent):
    return ''.join('%s<v8:Type>%s</v8:Type>\n' % (indent, r) for r in refs)


def multistate(indent):
    return ('%s<xr:PropertyState>\n%s\t<xr:Property>Type</xr:Property>\n%s\t<xr:State>MultiState</xr:State>\n%s</xr:PropertyState>\n'
            % (indent, indent, indent, indent))


def widened(check, extend, indent):
    out = '%s<Type xsi:type="xr:ExtendedProperty">\n' % indent
    if check:
        out += '%s\t<xr:CheckValue xsi:type="v8:TypeDescription">\n%s%s\t</xr:CheckValue>\n' % (indent, check, indent)
    out += '%s\t<xr:ExtendValue xsi:type="v8:TypeDescription">\n%s%s\t</xr:ExtendValue>\n%s</Type>\n' % (
        indent, extend, indent, indent)
    return out


def defined_type(uuid, name, seed, type_xml, adopt_from=None):
    head = ('\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>%s</Name>\n\t\t\t<Comment/>\n'
            '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % (name, adopt_from)
            if adopt_from else '\t\t\t<Name>%s</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n' % name)
    state = multistate('\t\t\t') if 'ExtendedProperty' in type_xml else ''
    return md('\t<DefinedType uuid="%s">\n\t\t<InternalInfo>\n%s\n%s\t\t</InternalInfo>\n\t\t<Properties>\n%s%s\t\t</Properties>\n\t</DefinedType>'
              % (uuid, gen([('DefinedType.' + name, 'DefinedType')], seed), state, head, type_xml))


def catalog(uuid, name, seed, children='', adopt_from=None):
    head = ('\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>%s</Name>\n\t\t\t<Comment/>\n'
            '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % (name, adopt_from)
            if adopt_from else '\t\t\t<Name>%s</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n' % name)
    ch = '\t\t<ChildObjects>\n%s\t\t</ChildObjects>\n' % children if children else '\t\t<ChildObjects/>\n'
    return md('\t<Catalog uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n%s\t\t</Properties>\n%s\t</Catalog>'
              % (uuid, gen(catalog_cats(name), seed), head, ch))


def attribute(uuid, name, type_xml, adopt_from=None):
    if adopt_from:
        return ('\t\t\t<Attribute uuid="%s">\n\t\t\t\t<InternalInfo>\n%s\t\t\t\t</InternalInfo>\n\t\t\t\t<Properties>\n\t\t\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n'
                '\t\t\t\t\t<Name>%s</Name>\n\t\t\t\t\t<Comment/>\n\t\t\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n%s'
                '\t\t\t\t</Properties>\n\t\t\t</Attribute>\n' % (uuid, multistate('\t\t\t\t\t'), name, adopt_from, type_xml))
    return ('\t\t\t<Attribute uuid="%s">\n\t\t\t\t<Properties>\n\t\t\t\t\t<Name>%s</Name>\n\t\t\t\t\t<Synonym/>\n\t\t\t\t\t<Comment/>\n%s'
            '\t\t\t\t</Properties>\n\t\t\t</Attribute>\n' % (uuid, name, type_xml))


def filter_criterion(uuid, name, seed, type_xml, content, adopt_from=None):
    head = ('\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>%s</Name>\n\t\t\t<Comment/>\n'
            '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % (name, adopt_from)
            if adopt_from else '\t\t\t<Name>%s</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n' % name)
    items = ''.join('\t\t\t\t<xr:Item xsi:type="xr:MDObjectRef">%s</xr:Item>\n' % c for c in content)
    state = multistate('\t\t\t') if 'ExtendedProperty' in type_xml else ''
    content_xml = '\t\t\t<Content>\n%s\t\t\t</Content>\n' % items if items else '\t\t\t<Content/>\n'
    return md('\t<FilterCriterion uuid="%s">\n\t\t<InternalInfo>\n%s\n%s\t\t</InternalInfo>\n\t\t<Properties>\n%s%s'
              '%s\t\t</Properties>\n\t\t<ChildObjects/>\n\t</FilterCriterion>'
              % (uuid, gen([('FilterCriterionManager.' + name, 'Manager'), ('FilterCriterionList.' + name, 'List')], seed),
                 state, head, type_xml, content_xml))


ANY = '\t\t\t\t\t\t<v8:TypeSet>cfg:AnyIBRef</v8:TypeSet>\n'


def main():
    work = tempfile.mkdtemp(prefix='adopted-widened-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    # base
    write(os.path.join(base, 'Catalogs', 'Справочник.xml'), catalog(uid('b', 1), 'Справочник', 'c4000001', attribute(
        uid('b', 2), 'Ссылка1', '\t\t\t\t\t<Type>\n\t\t\t\t\t\t<v8:TypeSet>cfg:AnyIBRef</v8:TypeSet>\n\t\t\t\t\t</Type>\n')))
    write(os.path.join(base, 'DefinedTypes', 'Владелец.xml'), defined_type(uid('b', 3), 'Владелец', 'c4000002',
          '\t\t\t<Type>\n\t\t\t\t<v8:Type>cfg:CatalogRef.Справочник</v8:Type>\n\t\t\t</Type>\n'))
    write(os.path.join(base, 'DefinedTypes', 'Пустой.xml'), defined_type(uid('b', 4), 'Пустой', 'c4000003',
          '\t\t\t<Type>\n\t\t\t\t<v8:Type>cfg:CatalogRef.Справочник</v8:Type>\n\t\t\t</Type>\n'))
    write(os.path.join(base, 'FilterCriteria', 'Отбор.xml'), filter_criterion(uid('b', 5), 'Отбор', 'c4000004',
          '\t\t\t<Type>\n\t\t\t\t<v8:Type>cfg:CatalogRef.Справочник</v8:Type>\n\t\t\t</Type>\n', []))
    names = [('Catalog', 'Справочник'), ('DefinedType', 'Владелец'), ('DefinedType', 'Пустой'), ('FilterCriterion', 'Отбор')]
    props.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>', names)
    # extension: its own catalog, the adopted ones
    own = 'ТестРасширение_Свой'
    write(os.path.join(ext, 'Catalogs', own + '.xml'), catalog(uid('e', 10), own, 'd4000010', attribute(
        uid('e', 11), 'Отбор', '\t\t\t\t\t<Type>\n\t\t\t\t\t\t<v8:Type>cfg:CatalogRef.Справочник</v8:Type>\n\t\t\t\t\t</Type>\n')))
    write(os.path.join(ext, 'Catalogs', 'Справочник.xml'), catalog(uid('e', 1), 'Справочник', 'd4000001', attribute(
        uid('e', 2), 'Ссылка1', widened(ANY, '\t\t\t\t\t\t\t<v8:Type>cfg:CatalogRef.%s</v8:Type>\n' % own, '\t\t\t\t\t'), uid('b', 2)),
        uid('b', 1)))
    write(os.path.join(ext, 'DefinedTypes', 'Владелец.xml'), defined_type(uid('e', 3), 'Владелец', 'd4000002',
          widened('', '\t\t\t\t\t<v8:Type>cfg:CatalogRef.%s</v8:Type>\n' % own, '\t\t\t'), uid('b', 3)))
    write(os.path.join(ext, 'DefinedTypes', 'Пустой.xml'), defined_type(uid('e', 4), 'Пустой', 'd4000003',
          '\t\t\t<Type/>\n', uid('b', 4)))
    write(os.path.join(ext, 'FilterCriteria', 'Отбор.xml'), filter_criterion(uid('e', 5), 'Отбор', 'd4000004',
          widened('', '\t\t\t\t\t<v8:Type>cfg:CatalogRef.%s</v8:Type>\n' % own, '\t\t\t'),
          ['Catalog.%s.Attribute.Отбор' % own], uid('b', 5)))
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                       [('Catalog', own)] + names)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'widened')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    cfe = os.path.join(dst, 'input.cfe')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', cfe, '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    for rel in ['Catalogs/Справочник.xml', 'DefinedTypes/Владелец.xml', 'DefinedTypes/Пустой.xml', 'FilterCriteria/Отбор.xml']:
        os.makedirs(os.path.dirname(os.path.join(dst, rel)), exist_ok=True)
        shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
        print(rel)
        print(open(os.path.join(dump, rel), encoding='utf-8-sig').read().split('<Properties>', 1)[1][:1500])
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
