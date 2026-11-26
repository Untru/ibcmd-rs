"""Build the home page work area fixtures with the platform: the clean-room base
configuration with two common forms placed on the home page under each work
area template, beside the platform's own dump of Ext/HomePageWorkArea.xml.
Case `one_column_v85` is built and saved by 8.5.1.1529 and dumped by
8.3.27.2214; `one_column_top` is the shape 1Cv8_обф.cf stores.

    python make_home_page_fixtures.py [case...]

Output: tests/fixtures/external/home_page/<case>/{input.cf, HomePageWorkArea.xml, stored.txt}
and the stored blob of each case on stdout.
"""
import os, shutil, sys, zlib
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import v8c
import make_compat_form_fixtures as compat

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE27 = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
EXE85 = r'C:\Program Files\1cv8\8.5.1.1529\bin\1cv8.exe'
HOME_PAGE_ENTRY = '00000000-0000-0000-0000-000000000002.8'
FORMS = [('ФормаСлева', '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a11'),
         ('ФормаСправа', '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a12')]

HEAD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<HomePageWorkArea xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<WorkingAreaTemplate>%s</WorkingAreaTemplate>
'''
ITEM = '''		<Item>
			<Form>CommonForm.%s</Form>
			<Height>%s</Height>
			<Visibility>
				<xr:Common>%s</xr:Common>
			</Visibility>
		</Item>
'''


def column(tag, items):
    return '\t<%s>\n%s\t</%s>\n' % (tag, ''.join(ITEM % item for item in items), tag)


MA = '\t<MACommandInterfaceDisplays>%s</MACommandInterfaceDisplays>\n'
LEFT = [(FORMS[0][0], 10, 'true')]
RIGHT = [(FORMS[1][0], 7, 'false')]
CASES = {
    'one_column': (EXE27, HEAD % 'OneColumn' + column('Column', LEFT + RIGHT)),
    'two_equal': (EXE27, HEAD % 'TwoColumnsEqualWidth' + column('LeftColumn', LEFT) + column('RightColumn', RIGHT)),
    'two_variable': (EXE27, HEAD % 'TwoColumnsVariableWidth' + column('LeftColumn', LEFT) + column('RightColumn', RIGHT)),
    'one_column_v85': (EXE85, HEAD % 'OneColumn' + column('Column', LEFT + RIGHT)),
    # MACommandInterfaceDisplays: stored after the columns as 0 Top, 1 Bottom,
    # 2 None; None is not printed.
    'one_column_top': (EXE27, HEAD % 'OneColumn' + column('Column', LEFT) + MA % 'Top'),
    'two_equal_bottom': (EXE27, HEAD % 'TwoColumnsEqualWidth' + column('LeftColumn', LEFT) + column('RightColumn', []) + MA % 'Bottom'),
}


def add_form(src, form, uuid):
    os.makedirs(os.path.join(src, 'CommonForms', form, 'Ext', 'Form'))
    open(os.path.join(src, 'CommonForms', form + '.xml'), 'w', encoding='utf-8').write(compat.FORM_MD % (uuid, form))
    open(os.path.join(src, 'CommonForms', form, 'Ext', 'Form.xml'), 'w', encoding='utf-8').write(compat.FORM_XML)
    open(os.path.join(src, 'CommonForms', form, 'Ext', 'Form', 'Module.bsl'), 'w', encoding='utf-8-sig').write(compat.MODULE)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    anchor = '<Language>Русский</Language>'
    assert anchor in t
    open(cfg, 'w', encoding='utf-8-sig').write(t.replace(anchor, anchor + '\n\t\t\t<CommonForm>%s</CommonForm>' % form, 1))


def stored_blob(cf):
    for name, body in v8c.read(cf).items():
        try:
            text = body.decode('utf-8-sig')
        except Exception:
            continue
        if FORMS[0][1] in text and text.startswith('{1,') and '{"B",' in text:
            return name, text
    return None, None


def main():
    only = sys.argv[1:]
    work = os.path.join(HERE, '.fixture-work-home-page'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    dst_root = os.path.join(REPO, 'tests', 'fixtures', 'external', 'home_page')
    os.makedirs(dst_root, exist_ok=True)
    for case, (exe, xml) in CASES.items():
        if only and case not in only:
            continue
        w = os.path.join(work, case); os.makedirs(w)
        log = os.path.join(w, 'platform.log')
        src = os.path.join(w, 'src'); shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
        for form, uuid in FORMS:
            add_form(src, form, uuid)
        open(os.path.join(src, 'Ext', 'HomePageWorkArea.xml'), 'w', encoding='utf-8').write(xml + '</HomePageWorkArea>')
        dst = os.path.join(dst_root, case); shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        cf = os.path.join(dst, 'input.cf')
        ib = os.path.join(w, 'ib')
        v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpCfg', cf], log)
        if exe != EXE27:
            ib = os.path.join(w, 'ib27')
            v8dump._run(EXE27, ['CREATEINFOBASE', 'File="%s"' % ib], log)
            v8dump._run(EXE27, ['DESIGNER', '/F', ib, '/LoadCfg', cf], log)
        dump = os.path.join(w, 'dump')
        v8dump._run(EXE27, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
        shutil.copy(os.path.join(dump, 'Ext', 'HomePageWorkArea.xml'), dst)
        # The record the platform stored, for the compiler's byte check.
        open(os.path.join(dst, 'stored.txt'), 'wb').write(v8c.read(cf)[HOME_PAGE_ENTRY])
        print('ok', case, stored_blob(cf))
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
