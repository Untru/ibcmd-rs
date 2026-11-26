"""Build the clean-room external fixtures with the platform and capture the
native dumps the Rust tests compare against.

    python make_fixtures.py [8.3.27.2214]
"""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
FIXTURES = [('ТестОбработка', 'test_processor', '.epf'), ('ТестОтчет', 'test_report', '.erf')]


def main():
    ver = sys.argv[1] if len(sys.argv) > 1 else '8.3.27.2214'
    exe = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    work = os.path.join(HERE, '.fixture-work'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    ib, log = os.path.join(work, 'ib'), os.path.join(work, 'platform.log')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    for src_name, label, ext in FIXTURES:
        dst = os.path.join(REPO, 'tests', 'fixtures', 'external', label)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        binp = os.path.join(dst, 'input' + ext)
        src = os.path.join(HERE, 'fixture_src', src_name, src_name + '.xml')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadExternalDataProcessorOrReportFromFiles', src, binp], log)
        expected = os.path.join(dst, 'expected')
        os.makedirs(expected)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                          os.path.join(expected, src_name + '.xml'), binp], log)
        print('ok', label)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
