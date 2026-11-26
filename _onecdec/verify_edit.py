"""Platform acceptance of a `cf load` result: the platform reads the loaded
file and dumps back exactly the edited tree.

    python verify_edit.py <loaded file> <edited tree> [--extension NAME] [--check]

.cf: a fresh file infobase, /LoadCfg, /UpdateDBCfg, optionally /CheckModules,
/DumpConfigToFiles. .cfe: the same with -Extension NAME over the empty base
the infobase starts with (the way oracle_dump.py captures extensions).
.epf/.erf: /DumpExternalDataProcessorOrReportToFiles. The dump is compared
with the edited tree file by file; ConfigDumpInfo.xml is compared without its
configVersion values (a loaded entry gets a new generation), and the tree's
dot directories (.ibcmd) are ignored. Exit 0 when they match.
"""
import os, re, shutil, sys, tempfile
sys.stdout.reconfigure(encoding='utf-8')
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

PLATFORM = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
VERSION = re.compile(rb' configVersion="[^"]*"')
# The integration-service object the platform names anew on every load.
INTEGRATION = re.compile(rb'(<xr:ClassId>fb282519-d103-4dd3-bc12-cb271d631dfc</xr:ClassId>\s*<xr:ObjectId>)[^<]*')


def files(root):
    out = {}
    for dp, dirs, fs in os.walk(root):
        dirs[:] = [d for d in dirs if not d.startswith('.')]
        for f in fs:
            if f == '.complete':
                continue
            p = os.path.join(dp, f)
            rel = os.path.relpath(p, root).replace(os.sep, '/')
            data = open(p, 'rb').read()
            if rel == 'ConfigDumpInfo.xml':
                data = VERSION.sub(b'', data)
            if rel == 'Configuration.xml':
                data = INTEGRATION.sub(lambda m: m.group(1), data)
            out[rel] = data
    return out


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    loaded, tree = os.path.abspath(args[0]), os.path.abspath(args[1])
    ext = sys.argv[sys.argv.index('--extension') + 1] if '--extension' in sys.argv else None
    if ext and ext in args:
        args.remove(ext)
    kind = os.path.splitext(loaded)[1].lower()
    work = tempfile.mkdtemp(prefix='ibcmd-verify-edit-')
    log = os.path.join(work, 'platform.log')
    dump = os.path.join(work, 'dump')
    try:
        if kind in ('.epf', '.erf'):
            ib = os.path.join(work, 'ib')
            v8dump._run(PLATFORM, ['CREATEINFOBASE', 'File="%s"' % ib], log)
            os.makedirs(dump)
            # Named as the tree names the object: its root file.
            roots = [f for f in os.listdir(tree) if f.endswith('.xml')]
            xml = os.path.join(dump, roots[0] if len(roots) == 1 else
                               os.path.splitext(os.path.basename(loaded))[0] + '.xml')
            v8dump._run(PLATFORM, ['DESIGNER', '/F', ib,
                                   '/DumpExternalDataProcessorOrReportToFiles', xml, loaded], log)
        else:
            ib = os.path.join(work, 'ib')
            v8dump._run(PLATFORM, ['CREATEINFOBASE', 'File="%s"' % ib], log)
            extension = ['-Extension', ext] if kind == '.cfe' else []
            v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/LoadCfg', loaded] + extension, log)
            if kind == '.cf':
                v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
            if '--check' in sys.argv:
                v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/CheckModules', '-Server',
                                       '-ThinClient'] + extension, log)
            v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump] + extension, log)
        want, got = files(tree), files(dump)
        # A real configuration our export does not reproduce whole: expect
        # the platform's dump of the base with only the edited files taken
        # from the tree (--native <dump of the base> --edits a,b,...).
        if '--native' in sys.argv:
            want = files(sys.argv[sys.argv.index('--native') + 1])
            edits = sys.argv[sys.argv.index('--edits') + 1].split(',') if '--edits' in sys.argv else []
            tree_files = files(tree)
            for p in edits:
                if p in tree_files:
                    want[p] = tree_files[p]
                else:
                    want.pop(p, None)
            want.pop('ConfigDumpInfo.xml', None)
            got.pop('ConfigDumpInfo.xml', None)
        # An external object's dump names its root file after the file; the
        # tree after the object. Compare the sets the two share by content.
        differ = sorted(p for p in set(want) | set(got) if want.get(p) != got.get(p))
        text = open(log, encoding='utf-8', errors='replace').read() if os.path.exists(log) else ''
        print('files', len(want), 'differ', len(differ))
        for p in differ[:40]:
            print('  ', 'missing' if p not in got else 'extra' if p not in want else 'diff', p)
        if differ and text:
            print('platform log tail:', text[-1500:])
        sys.exit(1 if differ else 0)
    finally:
        if '--keep' in sys.argv:
            print('kept', work)
        else:
            shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
