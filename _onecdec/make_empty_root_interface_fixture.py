"""Build the empty root command interface fixture with the platform: the
clean-room base configuration loaded with an empty Ext/CommandInterface.xml
and Ext/MainSectionCommandInterface.xml. 8.3.27.2214 stores both as
`{7,0,0,0,0,0,0}`, names them in ConfigDumpInfo.xml and writes neither file.

    python make_empty_root_interface_fixture.py

Output: tests/fixtures/external/empty_root_interface/{input.cf, ConfigDumpInfo.xml, ext.txt}
where ext.txt lists the files of the platform's Ext/.
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
EMPTY = '\ufeff<?xml version="1.0" encoding="UTF-8"?>\n<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20"/>'


def main():
    work = os.path.join(HERE, '.fixture-work-empty-ci'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    src = os.path.join(work, 'src'); shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
    for name in ['CommandInterface.xml', 'MainSectionCommandInterface.xml']:
        open(os.path.join(src, 'Ext', name), 'w', encoding='utf-8').write(EMPTY)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'empty_root_interface')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cf')], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    shutil.copy(os.path.join(dump, 'ConfigDumpInfo.xml'), dst)
    ext = sorted(os.listdir(os.path.join(dump, 'Ext')))
    open(os.path.join(dst, 'ext.txt'), 'w', encoding='utf-8', newline='\n').write('\n'.join(ext) + '\n')
    print('ok', ext)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
