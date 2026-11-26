"""A configuration whose DefaultStyle names its own style (root tuple field 9).

    python make_default_style_fixture.py

The clean-room base configuration plus a style `Основной` set as the default
style; the platform (8.3.27.2214) loads the XML, saves `input.cf` and dumps
Configuration.xml (expected: `<DefaultStyle>Style.Основной</DefaultStyle>`).

Output: tests/fixtures/external/default_style/{input.cf, Configuration.xml}
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src', 'cfe_base')
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
DST = os.path.join(REPO, 'tests', 'fixtures', 'external', 'default_style')

STYLE = ('<?xml version="1.0" encoding="UTF-8"?>\n<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" '
         'xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">\n'
         '<Style uuid="01a76cca-0000-4000-8000-00000000c0de"><Properties><Name>Основной</Name><Synonym/>'
         '<Comment/></Properties></Style></MetaDataObject>')
STYLE_BODY = ('<?xml version="1.0" encoding="UTF-8"?>\n<Style xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" '
              'xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" version="2.20">\n'
              '\t<Item name="FormBackColor">\n\t\t<Color>web:Cream</Color>\n\t</Item>\n</Style>')


def main():
    work = tempfile.mkdtemp(prefix='default-style-fixture-')
    src = os.path.join(work, 'src'); shutil.copytree(SRC, src)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    old = '<ChildObjects>\n\t\t\t<Language>Русский</Language>\n\t\t</ChildObjects>'
    assert old in t
    t = t.replace(old, '<ChildObjects><Language>Русский</Language><Style>Основной</Style></ChildObjects>')
    assert '<DefaultStyle/>' in t
    t = t.replace('<DefaultStyle/>', '<DefaultStyle>Style.Основной</DefaultStyle>')
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    os.makedirs(os.path.join(src, 'Styles', 'Основной', 'Ext'))
    open(os.path.join(src, 'Styles', 'Основной.xml'), 'w', encoding='utf-8-sig').write(STYLE)
    open(os.path.join(src, 'Styles', 'Основной', 'Ext', 'Style.xml'), 'w', encoding='utf-8-sig').write(STYLE_BODY)
    shutil.rmtree(DST, ignore_errors=True); os.makedirs(DST)
    ib = os.path.join(work, 'ib'); log = os.path.join(work, 'log')
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(DST, 'input.cf')], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    shutil.copy(os.path.join(dump, 'Configuration.xml'), DST)
    got = open(os.path.join(DST, 'Configuration.xml'), encoding='utf-8-sig').read()
    assert '<DefaultStyle>Style.Основной</DefaultStyle>' in got
    print('ok')
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
