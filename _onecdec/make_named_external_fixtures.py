"""Build external data processors whose names collide with what the export
lays out -- `DataProcessors` (the pipeline's own folder for them) and
`ConfigDumpInfo` (the file the adapter removes) -- from the clean-room
`ТестОбработка` sources renamed, with the platform's own dumps.

    python make_named_external_fixtures.py

Output: tests/fixtures/external/named_<name>/{input.epf, expected/}
"""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
SOURCE = 'ТестОбработка'
NAMES = ['DataProcessors', 'ConfigDumpInfo']


def renamed_copy(src_root, dst_root, old, new):
    for dp, _, fs in os.walk(src_root):
        rel = os.path.relpath(dp, src_root)
        target_dir = os.path.join(dst_root, rel.replace(old, new) if rel != '.' else '')
        os.makedirs(target_dir, exist_ok=True)
        for f in fs:
            data = open(os.path.join(dp, f), 'rb').read()
            if f.endswith(('.xml', '.bsl', '.html')):
                data = data.decode('utf-8-sig').replace(old, new).encode('utf-8-sig')
            open(os.path.join(target_dir, f.replace(old, new)), 'wb').write(data)


def main():
    work = os.path.join(HERE, '.fixture-work-named'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    ib, log = os.path.join(work, 'ib'), os.path.join(work, 'platform.log')
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    for name in NAMES:
        src = os.path.join(work, 'src_' + name)
        renamed_copy(os.path.join(HERE, 'fixture_src', SOURCE), src, SOURCE, name)
        dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'named_' + name)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        binp = os.path.join(dst, 'input.epf')
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadExternalDataProcessorOrReportFromFiles',
                          os.path.join(src, name + '.xml'), binp], log)
        expected = os.path.join(dst, 'expected'); os.makedirs(expected)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                          os.path.join(expected, name + '.xml'), binp], log)
        print('ok', name, sorted(os.listdir(expected)))
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
