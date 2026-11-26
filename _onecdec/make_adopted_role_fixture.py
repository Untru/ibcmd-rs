"""Build the adopted-role fixture with the platform: a base configuration
with a catalog and a role, and an extension adopting both, the role's rights
extended (granting rights on the catalog and on one standard attribute).

    python make_adopted_role_fixture.py [attrs_default]

The platform stores setForAttributesByDefault 2 for an adopted role whichever
value the source gives (both probed) and dumps false.

`attrs_default` (true/false, default false) is the extension role's
setForAttributesByDefault.
Output: tests/fixtures/external/adopted/role/{input.cfe, Roles/...}
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
RIGHTS_HEAD = ('\ufeff<?xml version="1.0" encoding="UTF-8"?>\n<Rights xmlns="http://v8.1c.ru/8.2/roles" '
               'xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
               'xsi:type="Rights" version="2.20">\n')


def uid(side, n):
    return '%sb000000-0000-4000-8000-%012d' % (side, n)


def role(uuid, head, info=''):
    return md('\t<Role uuid="%s">\n%s\t\t<Properties>\n%s\t\t</Properties>\n\t</Role>' % (uuid, info, head))


def rights(new, attrs, objects):
    body = ''
    for name, pairs in objects:
        body += '\t<object>\n\t\t<name>%s</name>\n' % name
        for right, value in pairs:
            body += '\t\t<right>\n\t\t\t<name>%s</name>\n\t\t\t<value>%s</value>\n\t\t</right>\n' % (right, value)
        body += '\t</object>\n'
    return (RIGHTS_HEAD + '\t<setForNewObjects>%s</setForNewObjects>\n\t<setForAttributesByDefault>%s</setForAttributesByDefault>\n'
            '\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\n%s</Rights>' % (new, attrs, body))


def main():
    attrs = sys.argv[1] if len(sys.argv) > 1 else 'false'
    work = tempfile.mkdtemp(prefix='adopted-role-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    write(os.path.join(base, 'Catalogs', 'Справочник.xml'), sub.catalog(uid('b', 1), 'Справочник', 'cb000001'))
    write(os.path.join(base, 'Roles', 'Роль.xml'), role(uid('b', 2), '\t\t\t<Name>Роль</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'))
    write(os.path.join(base, 'Roles', 'Роль', 'Ext', 'Rights.xml'), rights('false', 'true', []))
    props.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>',
                       [('Role', 'Роль'), ('Catalog', 'Справочник')])
    write(os.path.join(ext, 'Catalogs', 'Справочник.xml'), sub.catalog(uid('e', 1), 'Справочник', 'db000001').replace(
        '\t\t\t<Name>Справочник</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n',
        '\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>Справочник</Name>\n\t\t\t<Comment/>\n'
        '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % uid('b', 1)))
    write(os.path.join(ext, 'Roles', 'Роль.xml'), role(uid('e', 2),
          '\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>Роль</Name>\n\t\t\t<Comment/>\n'
          '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % uid('b', 2),
          '\t\t<InternalInfo>\n\t\t\t<xr:PropertyState>\n\t\t\t\t<xr:Property>Rights</xr:Property>\n'
          '\t\t\t\t<xr:State>Extended</xr:State>\n\t\t\t</xr:PropertyState>\n\t\t</InternalInfo>\n'))
    write(os.path.join(ext, 'Roles', 'Роль', 'Ext', 'Rights.xml'), rights('false', attrs, [
        ('Catalog.Справочник', [('Read', 'true'), ('View', 'true')]),
        ('Catalog.Справочник.StandardAttribute.Description', [('View', 'true')]),
    ]))
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                       [('Role', 'Роль'), ('Catalog', 'Справочник')])
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'role')
    shutil.rmtree(dst, ignore_errors=True)
    os.makedirs(os.path.join(dst, 'Roles', 'Роль', 'Ext'))
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    for rel in ['Roles/Роль.xml', 'Roles/Роль/Ext/Rights.xml']:
        shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
    print(open(os.path.join(dst, 'Roles/Роль/Ext/Rights.xml'), encoding='utf-8-sig').read())
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
