"""Build extension-root fixtures with the platform: variants of the clean-room
extension ТестРасширение whose root properties, controlled properties and
extended modules differ, each beside the platform's own dump of its
Configuration.xml.

    python make_extension_root_fixtures.py [8.3.27.2214]

Output: tests/fixtures/external/extension_roots/<case>/{input.cfe, Configuration.xml, ConfigDumpInfo.xml}
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
EXT = 'ТестРасширение'
ROLE = 'ТестРасширение_Роль'


def loc(text):
    return ('\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>ru</v8:lang>\n\t\t\t\t\t<v8:content>%s</v8:content>'
            '\n\t\t\t\t</v8:item>\n\t\t\t' % text)


def purposes(*values):
    items = ''.join('\n\t\t\t\t<v8:Value xsi:type="app:ApplicationUsePurpose">%s</v8:Value>' % v for v in values)
    return '<UsePurposes>%s\n\t\t\t</UsePurposes>' % items


CASES = {
    # every root value away from its default
    'values': [
        ('set', 'Comment', '<Comment>Комментарий "в кавычках" &amp; амперсанд</Comment>'),
        ('set', 'ConfigurationExtensionPurpose', '<ConfigurationExtensionPurpose>AddOn</ConfigurationExtensionPurpose>'),
        ('set', 'DefaultRunMode', '<DefaultRunMode>OrdinaryApplication</DefaultRunMode>'),
        ('set', 'UsePurposes', purposes('PlatformApplication', 'MobilePlatformApplication')),
        ('set', 'ScriptVariant', '<ScriptVariant>English</ScriptVariant>'),
        ('set', 'Vendor', '<Vendor>Поставщик</Vendor>'),
        ('set', 'Version', '<Version>1.2.3.4</Version>'),
        ('set', 'BriefInformation', '<BriefInformation>%s</BriefInformation>' % loc('Кратко')),
        ('set', 'DetailedInformation', '<DetailedInformation>%s</DetailedInformation>' % loc('Подробно')),
        ('set', 'Copyright', '<Copyright>%s</Copyright>' % loc('Авторы')),
        ('set', 'VendorInformationAddress', '<VendorInformationAddress>%s</VendorInformationAddress>' % loc('https://vendor')),
        ('set', 'ConfigurationInformationAddress', '<ConfigurationInformationAddress>%s</ConfigurationInformationAddress>' % loc('https://config')),
        ('set', 'InterfaceCompatibilityMode', '<InterfaceCompatibilityMode>Taxi</InterfaceCompatibilityMode>'),
    ],
    # the other run mode / interface / purpose spellings, a patch that does
    # not keep the mapping by ids
    'spellings': [
        ('set', 'ConfigurationExtensionPurpose', '<ConfigurationExtensionPurpose>Patch</ConfigurationExtensionPurpose>'),
        ('set', 'KeepMappingToExtendedConfigurationObjectsByIDs', '<KeepMappingToExtendedConfigurationObjectsByIDs>false</KeepMappingToExtendedConfigurationObjectsByIDs>'),
        ('set', 'DefaultRunMode', '<DefaultRunMode>Auto</DefaultRunMode>'),
        ('set', 'UsePurposes', purposes('MobilePlatformApplication')),
        ('set', 'InterfaceCompatibilityMode', '<InterfaceCompatibilityMode>Version8_2</InterfaceCompatibilityMode>'),
        ('set', 'ConfigurationExtensionCompatibilityMode', '<ConfigurationExtensionCompatibilityMode>Version8_3_14</ConfigurationExtensionCompatibilityMode>'),
    ],
    # nothing controlled, every root module and both command interfaces extended
    'modules': [
        ('drop', 'DefaultRunMode'), ('drop', 'UsePurposes'), ('drop', 'DefaultRoles'),
        ('drop', 'DefaultLanguage'), ('drop', 'InterfaceCompatibilityMode'),
        ('module', 'ManagedApplicationModule'), ('module', 'SessionModule'),
        ('module', 'ExternalConnectionModule'), ('module', 'OrdinaryApplicationModule'),
        ('state', 'CommandInterface'), ('state', 'MainSectionCommandInterface'),
    ],
    # an own role among the default roles
    'roles': [
        ('role',),
        ('set', 'DefaultRoles', '<DefaultRoles>\n\t\t\t\t<xr:Item xsi:type="xr:MDObjectRef">Role.%s</xr:Item>\n\t\t\t</DefaultRoles>' % ROLE),
    ],
}

ROLE_XML = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Role uuid="7d3c2b1a-0f1e-4d2c-9b8a-112233445566">
		<Properties>
			<Name>%s</Name>
			<Synonym/>
			<Comment/>
		</Properties>
	</Role>
</MetaDataObject>''' % ROLE

RIGHTS_XML = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="Rights" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
</Rights>'''


def property_state(name):
    return ('\n\t\t\t<xr:PropertyState>\n\t\t\t\t<xr:Property>%s</xr:Property>\n\t\t\t\t'
            '<xr:State>Extended</xr:State>\n\t\t\t</xr:PropertyState>' % name)


def apply(src, ops):
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    for op in ops:
        kind = op[0]
        if kind == 'set':
            t, n = re.subn(r'<%s(?:/>|>.*?</%s>)' % (op[1], op[1]), lambda _: op[2], t, count=1, flags=re.S)
            assert n == 1, op
        elif kind == 'drop':
            t, n = re.subn(r'\s*<%s(?:/>|>.*?</%s>)' % (op[1], op[1]), '', t, count=1, flags=re.S)
            assert n == 1, op
        elif kind in ('module', 'state'):
            t = t.replace('\t\t</InternalInfo>', property_state(op[1]) + '\n\t\t</InternalInfo>', 1)
            if kind == 'module':
                os.makedirs(os.path.join(src, 'Ext'), exist_ok=True)
                open(os.path.join(src, 'Ext', op[1] + '.bsl'), 'w', encoding='utf-8-sig').write('// %s\n' % op[1])
        elif kind == 'role':
            t = t.replace('<CommonModule>ТестРасширение_Модуль</CommonModule>',
                          '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t<Role>%s</Role>' % ROLE)
            os.makedirs(os.path.join(src, 'Roles', ROLE, 'Ext'))
            open(os.path.join(src, 'Roles', ROLE + '.xml'), 'w', encoding='utf-8').write(ROLE_XML)
            open(os.path.join(src, 'Roles', ROLE, 'Ext', 'Rights.xml'), 'w', encoding='utf-8').write(RIGHTS_XML)
    open(cfg, 'w', encoding='utf-8-sig').write(t)


def build(exe, work, case, ops, dst_root, builder=None):
    """`builder` (another platform) saves the .cfe; `exe` always dumps it."""
    case_work = os.path.join(work, case); os.makedirs(case_work)
    log = os.path.join(case_work, 'platform.log'); ib = os.path.join(case_work, 'ib')
    src = os.path.join(case_work, 'src'); shutil.copytree(os.path.join(SRC, EXT), src)
    apply(src, ops)
    make = builder or exe
    v8dump._run(make, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(make, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
    v8dump._run(make, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(make, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', EXT], log)
    built = os.path.join(case_work, 'input.cfe')
    v8dump._run(make, ['DESIGNER', '/F', ib, '/DumpCfg', built, '-Extension', EXT], log)
    if builder:
        ib = os.path.join(case_work, 'ib-dump')
        v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadCfg', built, '-Extension', EXT], log)
    dump = os.path.join(case_work, 'dump')
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    dst = os.path.join(dst_root, case); os.makedirs(dst)
    shutil.copy(built, dst)
    shutil.copy(os.path.join(dump, 'Configuration.xml'), dst)
    shutil.copy(os.path.join(dump, 'ConfigDumpInfo.xml'), dst)
    print('ok', case)


def main():
    ver = sys.argv[1] if len(sys.argv) > 1 else '8.3.27.2214'
    exe = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    work = os.path.join(HERE, '.fixture-work-roots'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    dst_root = os.path.join(REPO, 'tests', 'fixtures', 'external', 'extension_roots')
    shutil.rmtree(dst_root, ignore_errors=True); os.makedirs(dst_root)
    for case, ops in CASES.items():
        build(exe, work, case, ops, dst_root)
    # saved by 8.5 (a `{76,…}` root tuple), dumped by `exe`
    exe85 = r'C:\Program Files\1cv8\8.5.1.1529\bin\1cv8.exe'
    build(exe, work, 'values_v85', CASES['values'], dst_root, builder=exe85)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
