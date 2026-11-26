"""Build role fixtures with the platform: the clean-room base configuration
with one role whose configuration rights set ExclusiveModeTerminationAtSessionStart
against the role's "set for new objects" flag, beside the platform's own dump
of the role's rights.

    python make_role_rights_fixtures.py

Output: tests/fixtures/external/role_rights/<case>/{input.cf, Rights.xml}
  new_true_emt_false -- setForNewObjects true, the right false
  new_false_emt_true -- setForNewObjects false, the right true (ИТК's case)
"""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_compat_form_fixtures as compat

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
ROLE = 'РольПроверка'
ROLE_UUID = '6a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d'

ROLE_MD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<Role uuid="%s">
		<Properties>
			<Name>%s</Name>
			<Synonym/>
			<Comment/>
		</Properties>
	</Role>
</MetaDataObject>''' % (ROLE_UUID, ROLE)

RIGHTS = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="Rights" version="2.20">
	<setForNewObjects>%s</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>Configuration.БазаТест</name>
		<right>
			<name>ExclusiveModeTerminationAtSessionStart</name>
			<value>%s</value>
		</right>
	</object>
</Rights>'''

CASES = {'new_true_emt_false': ('true', 'false'), 'new_false_emt_true': ('false', 'true')}


def main():
    work = os.path.join(HERE, '.fixture-work-roles'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    for case, (new_objects, value) in CASES.items():
        w = os.path.join(work, case); os.makedirs(w)
        log = os.path.join(w, 'platform.log'); ib = os.path.join(w, 'ib')
        src = os.path.join(w, 'src'); shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
        os.makedirs(os.path.join(src, 'Roles', ROLE, 'Ext'))
        open(os.path.join(src, 'Roles', ROLE + '.xml'), 'w', encoding='utf-8').write(ROLE_MD)
        open(os.path.join(src, 'Roles', ROLE, 'Ext', 'Rights.xml'), 'w', encoding='utf-8').write(RIGHTS % (new_objects, value))
        cfg = os.path.join(src, 'Configuration.xml')
        t = open(cfg, encoding='utf-8-sig').read()
        anchor = '<Language>Русский</Language>'
        assert anchor in t
        open(cfg, 'w', encoding='utf-8-sig').write(t.replace(anchor, anchor + '\n\t\t\t<Role>%s</Role>' % ROLE, 1))
        dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'role_rights', case)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cf')], log)
        dump = os.path.join(w, 'dump')
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
        shutil.copy(os.path.join(dump, 'Roles', ROLE, 'Ext', 'Rights.xml'), dst)
        text = open(os.path.join(dst, 'Rights.xml'), encoding='utf-8-sig').read()
        print('ok', case, 'ExclusiveModeTermination' in text)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
