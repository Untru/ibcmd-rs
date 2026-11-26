"""Build the adopted-exchange-plan fixture with the platform: a base
configuration with an exchange plan holding one catalog, and an extension
adopting the plan and adding two catalogs of its own to its content.

    python make_adopted_exchange_plan_fixture.py

Output: tests/fixtures/external/adopted/exchange_plan/{input.cfe, ExchangePlans/...}
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted
import make_adopted_properties_fixture as props
import make_adopted_subscription_fixture as sub

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT, write, md = adopted.EXE, adopted.EXT, adopted.write, props.md
OWN = ['ТестРасширение_Второй', 'ТестРасширение_Первый']
HEAD = ('\ufeff<?xml version="1.0" encoding="UTF-8"?>\n<ExchangePlanContent xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" '
        'xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">\n')


def uid(side, n):
    return '%s9000000-0000-4000-8000-%012d' % (side, n)


def plan(uuid, seed, head):
    cats = [('ExchangePlan' + a, a) for a in ['Object', 'Ref', 'Selection', 'List', 'Manager']]
    return md('\t<ExchangePlan uuid="%s">\n\t\t<InternalInfo>\n\t\t\t<xr:ThisNode>%s</xr:ThisNode>\n%s\n\t\t</InternalInfo>\n'
              '\t\t<Properties>\n%s\t\t</Properties>\n\t\t<ChildObjects/>\n\t</ExchangePlan>'
              % (uuid, uuid[:-3] + '999', props.generated('ExchangePlan', 'План', seed, cats), head))


def main():
    work = tempfile.mkdtemp(prefix='adopted-exchange-plan-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    write(os.path.join(base, 'Catalogs', 'Справочник.xml'), sub.catalog(uid('b', 1), 'Справочник', 'c9000001'))
    write(os.path.join(base, 'ExchangePlans', 'План.xml'), plan(uid('b', 2), 'c9000002',
          '\t\t\t<Name>План</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'))
    write(os.path.join(base, 'ExchangePlans', 'План', 'Ext', 'Content.xml'), HEAD +
          '\t<Item>\n\t\t<Metadata>Catalog.Справочник</Metadata>\n\t\t<AutoRecord>Allow</AutoRecord>\n\t</Item>\n</ExchangePlanContent>')
    props.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>',
                       [('Catalog', 'Справочник'), ('ExchangePlan', 'План')])
    for i, name in enumerate(OWN, 1):
        write(os.path.join(ext, 'Catalogs', name + '.xml'), sub.catalog(uid('e', 10 + i), name, 'd900001%d' % i))
    write(os.path.join(ext, 'ExchangePlans', 'План.xml'), plan(uid('e', 2), 'd9000002',
          '\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>План</Name>\n\t\t\t<Comment/>\n'
          '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % uid('b', 2)).replace(
        '\t\t</InternalInfo>', '\t\t\t<xr:PropertyState>\n\t\t\t\t<xr:Property>Content</xr:Property>\n'
        '\t\t\t\t<xr:State>Extended</xr:State>\n\t\t\t</xr:PropertyState>\n\t\t</InternalInfo>'))
    items = ''.join('\t\t<Item>\n\t\t\t<Metadata>Catalog.%s</Metadata>\n\t\t\t<State>Modify</State>\n\t\t</Item>\n' % n for n in OWN)
    write(os.path.join(ext, 'ExchangePlans', 'План', 'Ext', 'Content.xml'), HEAD +
          '\t<Item>\n\t\t<Metadata>Catalog.%s</Metadata>\n\t\t<AutoRecord>Deny</AutoRecord>\n\t</Item>\n'
          '\t<ExtensionProperty>\n%s\t</ExtensionProperty>\n</ExchangePlanContent>' % (OWN[0], items))
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                       [('Catalog', n) for n in OWN] + [('ExchangePlan', 'План')])
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'exchange_plan')
    shutil.rmtree(dst, ignore_errors=True)
    os.makedirs(os.path.join(dst, 'ExchangePlans', 'План', 'Ext'))
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    for rel in ['ExchangePlans/План.xml', 'ExchangePlans/План/Ext/Content.xml']:
        shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
    print(open(os.path.join(dst, 'ExchangePlans/План/Ext/Content.xml'), encoding='utf-8-sig').read())
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
