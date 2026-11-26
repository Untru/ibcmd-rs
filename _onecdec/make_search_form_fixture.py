"""Build the default-search-form fixture with the platform: the clean-room base
configuration with one common form set as its DefaultSearchForm, beside the
platform's own dump of Configuration.xml.

    python make_search_form_fixture.py

Output: tests/fixtures/external/search_form/{input.cf, Configuration.xml}
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
FORM = 'ФормаПоиска'


def main():
    work = os.path.join(HERE, '.fixture-work-search'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    src = os.path.join(work, 'src'); shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
    compat.add_form(src, FORM, '<Language>Русский</Language>', '<Language>Русский</Language>\n\t\t\t<CommonForm>%s</CommonForm>')
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    assert '<DefaultSearchForm/>' in t
    t = t.replace('<DefaultSearchForm/>', '<DefaultSearchForm>CommonForm.%s</DefaultSearchForm>' % FORM)
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'search_form')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cf')], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    shutil.copy(os.path.join(dump, 'Configuration.xml'), dst)
    print('ok', [l.strip() for l in open(os.path.join(dst, 'Configuration.xml'), encoding='utf-8-sig') if 'DefaultSearchForm' in l])
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
