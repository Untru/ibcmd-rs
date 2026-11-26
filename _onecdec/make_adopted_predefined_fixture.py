"""Build the adopted-predefined fixture with the platform: a base catalog
with one predefined item, and an extension adopting the catalog with its
predefined items extended -- the base item adopted, one item of its own.

    python make_adopted_predefined_fixture.py

Output: tests/fixtures/external/adopted/predefined/{input.cfe, Catalogs/...}
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
HEAD = ('\ufeff<?xml version="1.0" encoding="UTF-8"?>\n<PredefinedData xmlns="http://v8.1c.ru/8.3/xcf/predef" '
        'xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" '
        'xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        'xsi:type="CatalogPredefinedItems" version="2.20">\n')


def uid(side, n):
    return '%sa000000-0000-4000-8000-%012d' % (side, n)


def item(uuid, name, code, state=None):
    extra = '\t\t<ExtensionState>%s</ExtensionState>\n' % state if state else ''
    return ('\t<Item id="%s">\n\t\t<Name>%s</Name>\n\t\t<Code>%s</Code>\n\t\t<Description>%s</Description>\n'
            '\t\t<IsFolder>false</IsFolder>\n%s\t</Item>\n' % (uuid, name, code, name, extra))


def catalog(uuid, seed, head, info=''):
    cats = [('Catalog' + a, a) for a in ['Object', 'Ref', 'Selection', 'List', 'Manager']]
    return md('\t<Catalog uuid="%s">\n\t\t<InternalInfo>\n%s\n%s\t\t</InternalInfo>\n\t\t<Properties>\n%s\t\t</Properties>\n'
              '\t\t<ChildObjects/>\n\t</Catalog>' % (uuid, props.generated('Catalog', 'Справочник', seed, cats), info, head))


def main():
    work = tempfile.mkdtemp(prefix='adopted-predefined-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    write(os.path.join(base, 'Catalogs', 'Справочник.xml'), catalog(uid('b', 1), 'ca000001',
          '\t\t\t<Name>Справочник</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'))
    write(os.path.join(base, 'Catalogs', 'Справочник', 'Ext', 'Predefined.xml'),
          HEAD + item(uid('b', 2), 'Базовый', '000000001') + '</PredefinedData>')
    props.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>', [('Catalog', 'Справочник')])
    write(os.path.join(ext, 'Catalogs', 'Справочник.xml'), catalog(uid('e', 1), 'da000001',
          '\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>Справочник</Name>\n\t\t\t<Comment/>\n'
          '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % uid('b', 1),
          '\t\t\t<xr:PropertyState>\n\t\t\t\t<xr:Property>Predefined</xr:Property>\n\t\t\t\t<xr:State>Extended</xr:State>\n'
          '\t\t\t</xr:PropertyState>\n'))
    write(os.path.join(ext, 'Catalogs', 'Справочник', 'Ext', 'Predefined.xml'),
          HEAD + item(uid('b', 2), 'Базовый', '000000001', 'AdoptedCheck')
          + item(uid('e', 3), 'ТестРасширение_Свой', '000000002', 'Native') + '</PredefinedData>')
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                       [('Catalog', 'Справочник')])
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'predefined')
    shutil.rmtree(dst, ignore_errors=True)
    os.makedirs(os.path.join(dst, 'Catalogs', 'Справочник', 'Ext'))
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    for rel in ['Catalogs/Справочник.xml', 'Catalogs/Справочник/Ext/Predefined.xml']:
        shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
    print(open(os.path.join(dst, 'Catalogs/Справочник/Ext/Predefined.xml'), encoding='utf-8-sig').read())
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
