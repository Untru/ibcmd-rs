"""8.5 oracle dumps of the extension fixtures: every .cfe already under
tests/fixtures/external, loaded by 8.5.1.1529 over its base configuration and
dumped by that platform (Designer XML, format 2.21). The test compares our
`cf export --source-version 2.21` of the same input.cfe to the whole tree.

    python make_v85_extension_fixtures.py [case ...]     # default: every case

Output: tests/fixtures/external/v85_extension/<case>/ (the platform's dump;
the input stays where it is, see CASES). Each platform run happens in a fresh
temp dir (other agents run the platform concurrently).
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted
import make_adopted_children_fixture as children

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
FIX = os.path.join(REPO, 'tests', 'fixtures', 'external')
EXT = 'ТестРасширение'
EXE85 = r'C:\Program Files\1cv8\8.5.1.1529\bin\1cv8.exe'


def cfe_base(root):
    shutil.copytree(os.path.join(SRC, 'cfe_base'), root)


def adopted_base(case):
    return lambda root: adopted.base_sources(root, adopted.CASES[case])


def children_base(root):
    """The base of adopted/document_children (make_adopted_children_fixture.py)."""
    c = children
    shutil.copytree(os.path.join(SRC, 'cfe_base'), root)
    c.write(os.path.join(root, 'Documents', 'Документ.xml'), c.document(c.BASE_DOC, 'd0000001', children='\n'.join([
        c.attribute(c.BASE_ATTR, 'Реквизит', c.STRING),
        c.tabular(c.BASE_TS, 'd0000002', c.attribute(c.BASE_TS_ATTR, 'Количество', c.NUMBER, indent='\t\t\t\t\t'))])))
    c.write(os.path.join(root, 'Documents', 'Документ', 'Ext', 'ObjectModule.bsl'),
            'Процедура ПередЗаписью(Отказ)\nКонецПроцедуры\n', True)
    cfg = os.path.join(root, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    open(cfg, 'w', encoding='utf-8-sig').write(
        t.replace('<Language>Русский</Language>', '<Language>Русский</Language>\n\t\t\t<Document>Документ</Document>', 1))


def loc(*items):
    return ''.join('\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>%s</v8:lang>\n\t\t\t\t\t<v8:content>%s</v8:content>'
                   '\n\t\t\t\t</v8:item>' % item for item in items) + '\n\t\t\t'


def captions_source(root):
    """The 8.5 dump of test_extension with the root's Caption and ShortCaption
    set: an extension saved by 8.5 (`{76,…}` root tuple) carrying both."""
    shutil.copytree(os.path.join(FIX, 'v85_extension', 'test_extension'), root)
    cfg = os.path.join(root, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<Caption/>', '<Caption>%s</Caption>' % loc(('ru', 'Заголовок "в кавычках" &amp; &lt;угол&gt;'), ('en', 'Caption')), 1)
    t = t.replace('<ShortCaption/>', '<ShortCaption>%s</ShortCaption>' % loc(('ru', 'Кратко')), 1)
    open(cfg, 'w', encoding='utf-8-sig').write(t)


# Inputs 8.5 saves itself (no 8.3.27 fixture holds them).
BUILT = {
    'roots_captions_v85': (captions_source, cfe_base),
}


def built_input(case, work, log):
    """Saves the extension of `case` with 8.5 into v85_extension_inputs/<case>/input.cfe."""
    source_of, base_of = BUILT[case]
    ib = os.path.join(work, 'ib-build')
    base = os.path.join(work, 'base-build'); base_of(base)
    src = os.path.join(work, 'src'); source_of(src)
    v8dump._run(EXE85, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', EXT], log)
    dst = os.path.join(FIX, 'v85_extension_inputs', case)
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)


# case -> (input.cfe relative to tests/fixtures/external, base builder)
CASES = {
    'roots_captions_v85': ('v85_extension_inputs/roots_captions_v85/input.cfe', cfe_base),
    'test_extension': ('test_extension/input.cfe', cfe_base),
    'roots_values': ('extension_roots/values/input.cfe', cfe_base),
    'roots_spellings': ('extension_roots/spellings/input.cfe', cfe_base),
    'roots_modules': ('extension_roots/modules/input.cfe', cfe_base),
    'roots_roles': ('extension_roots/roles/input.cfe', cfe_base),
    'roots_values_v85': ('extension_roots/values_v85/input.cfe', cfe_base),
    'v85_form': ('v85_form/input.cfe', cfe_base),
    'form_events': ('form_events/input.cfe', cfe_base),
    'template_fonts': ('template_fonts/input.cfe', cfe_base),
    'adopted_document_children': ('adopted/document_children/input.cfe', children_base),
}
for _name in adopted.CASES:
    CASES['adopted_' + _name] = ('adopted/%s/input.cfe' % _name, adopted_base(_name))


def build(case):
    rel, base_of = CASES[case]
    work = tempfile.mkdtemp(prefix='v85ext-%s-' % case.replace('_', ''))
    try:
        log = os.path.join(work, 'platform.log')
        if case in BUILT:
            built_input(case, work, log)
        ib = os.path.join(work, 'ib')
        base = os.path.join(work, 'base'); base_of(base)
        v8dump._run(EXE85, ['CREATEINFOBASE', 'File="%s"' % ib], log)
        v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
        v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
        v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/LoadCfg', os.path.join(FIX, rel), '-Extension', EXT], log)
        dump = os.path.join(work, 'dump')
        v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
        dst = os.path.join(FIX, 'v85_extension', case)
        shutil.rmtree(dst, ignore_errors=True)
        shutil.copytree(dump, dst)
        n = sum(len(f) for _, _, f in os.walk(dst))
        print('ok    %-32s %4d files' % (case, n))
    except v8dump.DumpError as ex:
        print('FAIL  %-32s %s' % (case, str(ex)[:400]))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    sys.stdout.flush()


def main():
    for case in sys.argv[1:] or list(CASES):
        build(case)


if __name__ == '__main__':
    main()
