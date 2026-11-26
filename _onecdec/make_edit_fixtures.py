"""Edited trees dumped by the platform, for `cf load` tests of edits beyond
module text: the platform loads the edited XML (so the edit is one it
accepts) and dumps it; the test loads that tree onto the unedited file with
`cf load` and expects its export to be the platform's dump.

    python make_edit_fixtures.py

Cases (tests/fixtures/external/edits/<case>/tree, the extension's dump):
  ext_own_attribute  the adopted document of adopted/document_children gains
                     an attribute of the extension's own
"""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted
import make_adopted_children_fixture as children

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT = adopted.EXE, adopted.EXT
OWN_ATTR = 'e1000000-0000-4000-8000-000000000009'


def build_children(work, own_attribute):
    """The base configuration and extension of adopted/document_children,
    the extension's document with an own attribute when asked."""
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    adopted.write(os.path.join(base, 'Documents', 'Документ.xml'), children.document(
        children.BASE_DOC, 'd0000001', children='\n'.join([
            children.attribute(children.BASE_ATTR, 'Реквизит', children.STRING),
            children.tabular(children.BASE_TS, 'd0000002', children.attribute(
                children.BASE_TS_ATTR, 'Количество', children.NUMBER, indent='\t\t\t\t\t'))])))
    adopted.write(os.path.join(base, 'Documents', 'Документ', 'Ext', 'ObjectModule.bsl'),
                  'Процедура ПередЗаписью(Отказ)\nКонецПроцедуры\n', True)
    cfg = os.path.join(base, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    open(cfg, 'w', encoding='utf-8-sig').write(t.replace(
        '<Language>Русский</Language>', '<Language>Русский</Language>\n\t\t\t<Document>Документ</Document>', 1))
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    items = [
        children.attribute(children.EXT_ATTR, 'Реквизит', None, children.BASE_ATTR),
        children.tabular(children.EXT_TS, 'd0000004', children.attribute(
            children.EXT_TS_ATTR, 'Количество', children.NUMBER, children.BASE_TS_ATTR,
            indent='\t\t\t\t\t'), children.BASE_TS)]
    if own_attribute:
        items.insert(1, children.attribute(OWN_ATTR, 'ТестРасширение_НовыйРеквизит', children.STRING))
    adopted.write(os.path.join(ext, 'Documents', 'Документ.xml'), children.document(
        children.EXT_DOC, 'd0000003', children.BASE_DOC, '\n'.join(items)))
    adopted.write(os.path.join(ext, 'Documents', 'Документ', 'Ext', 'ObjectModule.bsl'),
                  '// расширение модуля объекта\n', True)
    cfg = os.path.join(ext, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    anchor = '<CommonModule>ТестРасширение_Модуль</CommonModule>'
    open(cfg, 'w', encoding='utf-8-sig').write(
        t.replace(anchor, anchor + '\n\t\t\t<Document>Документ</Document>', 1))
    return base, ext


def ext_own_attribute(work, dst):
    base, ext = build_children(work, own_attribute=True)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', os.path.join(dst, 'tree'), '-Extension', EXT], log)


CASES = {'ext_own_attribute': ext_own_attribute}


def main():
    for name, build in CASES.items():
        work = os.path.join(HERE, '.fixture-work-edit-' + name)
        shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
        dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'edits', name)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        build(work, dst)
        print('ok', name, sum(len(f) for _, _, f in os.walk(dst)), 'files')
        shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
