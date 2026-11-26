"""Derive edge-case variants of the clean-room processor fixture and capture
the platform's own dump of each as the expected tree.

    python make_variant_fixtures.py [8.3.27.2214]

Variants (tests/fixtures/external/variants/<name>/{input.epf,expected/}):
  no_copyinfo      the container without its copyinfo entry
  bom_only_module  the object module text is only a BOM
  no_module        the container without the object module entry
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump
import v8c

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(REPO, 'tests', 'fixtures', 'external', 'test_processor', 'input.epf')
OUT = os.path.join(REPO, 'tests', 'fixtures', 'external', 'variants')
NAME = 'ТестОбработка'


def without_version(e, entry):
    v = e['versions'].decode('utf-8-sig')
    v = re.sub(r',\s*"%s",\s*[0-9a-f-]{36}' % re.escape(entry), '', v)
    n = len(re.findall(r'"[^"]*",\s*[0-9a-f-]{36}', v))
    v = re.sub(r'^\{1,\d+,', '{1,%d,' % n, v)
    e['versions'] = b'\xef\xbb\xbf' + v.encode('utf-8')


def build_from_sources(exe, ib, log, work, module_text):
    """the clean-room sources with the object module replaced (None = removed),
    built into an .epf by the platform itself"""
    src = os.path.join(work, 'src'); shutil.rmtree(src, ignore_errors=True)
    shutil.copytree(os.path.join(HERE, 'fixture_src', NAME), src)
    module = os.path.join(src, NAME, 'Ext', 'ObjectModule.bsl')
    if module_text is None:
        os.remove(module)
    else:
        open(module, 'wb').write(module_text)
    out = os.path.join(work, 'built.epf')
    if os.path.exists(out): os.remove(out)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadExternalDataProcessorOrReportFromFiles',
                      os.path.join(src, NAME + '.xml'), out], log)
    return open(out, 'rb').read()


def variants(exe, ib, log, work):
    e = dict(v8c.read(SRC)); del e['copyinfo']; without_version(e, 'copyinfo')
    yield 'no_copyinfo', v8c.write15(e, deflate_all=True)
    yield 'bom_only_module', build_from_sources(exe, ib, log, work, bytes([0xEF, 0xBB, 0xBF]))
    yield 'no_module', build_from_sources(exe, ib, log, work, None)


def main():
    ver = sys.argv[1] if len(sys.argv) > 1 else '8.3.27.2214'
    exe = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    work = os.path.join(HERE, '.fixture-work'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    ib, log = os.path.join(work, 'ib'), os.path.join(work, 'platform.log')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    for label, container in variants(exe, ib, log, work):
        dst = os.path.join(OUT, label)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        binp = os.path.join(dst, 'input.epf')
        open(binp, 'wb').write(container)
        expected = os.path.join(dst, 'expected'); os.makedirs(expected)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                          os.path.join(expected, NAME + '.xml'), binp], log)
        print('ok', label, sorted(os.listdir(os.path.join(expected, NAME, 'Ext'))) if os.path.isdir(os.path.join(expected, NAME, 'Ext')) else '(no Ext)')
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
