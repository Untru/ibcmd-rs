"""Build the clean-room extension fixture (.cfe) with the platform and capture
its native dump.

    python make_cfe_fixture.py [8.3.27.2214]

fixture_src/cfe_base is an empty base configuration; fixture_src/ТестРасширение
an extension with one own common module (no adopted objects, so it applies
to the empty base). Output: tests/fixtures/external/test_extension/{input.cfe,expected/}.
"""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXT = 'ТестРасширение'


def main():
    ver = sys.argv[1] if len(sys.argv) > 1 else '8.3.27.2214'
    exe = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    work = os.path.join(HERE, '.fixture-work'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log')
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'test_extension')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    cfe = os.path.join(dst, 'input.cfe')
    ib = os.path.join(work, 'ib')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(HERE, 'fixture_src', 'cfe_base')], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(HERE, 'fixture_src', EXT),
                      '-Extension', EXT], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpCfg', cfe, '-Extension', EXT], log)
    # native dump of the built file, the way oracle_dump.py captures .cfe
    ib2 = os.path.join(work, 'ib2')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib2], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib2, '/LoadConfigFromFiles', os.path.join(HERE, 'fixture_src', 'cfe_base')], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib2, '/UpdateDBCfg'], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib2, '/LoadCfg', cfe, '-Extension', EXT], log)
    expected = os.path.join(dst, 'expected')
    v8dump._run(exe, ['DESIGNER', '/F', ib2, '/DumpConfigToFiles', expected, '-Extension', EXT], log)
    print('ok test_extension', sorted(os.listdir(expected)))
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
