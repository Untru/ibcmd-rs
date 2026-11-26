"""Platform acceptance of `ibcmd-rs cf load`.

    python verify_load.py corpus <corpus-root>        # every ext-bin/*.epf|*.erf
    python verify_load.py cf <file.cf> <sources-dir>   # decompiler round trip

corpus: export each object, append a comment to its object module and to the
first form module that exists, `cf load` onto the original, dump the result
with 8.3.27.2214 in the configuration infobase and compare with the edited
tree. A file counts as expected when it equals the edited tree; files the
exporter itself does not reproduce (not in the ratchet baseline) are skipped.

cf: export the .cf, replace every closed `<Kind>Module.bin` that has a
recovered source -- `<sources-dir>/<path of the .bin, with .bsl>` (a tree
mirroring the export) or, for a common module, `<sources-dir>/<ModuleName>.bsl`
-- with that text, `cf load`, load the result into a fresh infobase, run
/CheckModules and dump the configuration back; every dumped text must equal
the recovered source.

The binary under test is target/release/ibcmd-rs.exe of this repository, or
IBCMD_RS_EXE.
"""
import os, shutil, subprocess, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.environ.get('IBCMD_RS_EXE') or os.path.join(os.path.dirname(HERE), 'target', 'release', 'ibcmd-rs.exe')
PLATFORM = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
BOM = b'\xef\xbb\xbf'


def run(*args):
    r = subprocess.run([EXE, *args], capture_output=True)
    return r.returncode, (r.stdout or r.stderr).decode('utf-8', 'replace')


def files(root):
    out = {}
    for dp, _, fs in os.walk(root):
        for f in fs:
            if f == '.complete':
                continue
            p = os.path.join(dp, f)
            out[os.path.relpath(p, root).replace(os.sep, '/')] = p
    return out


def append_comment(path):
    data = open(path, 'rb').read()
    open(path, 'wb').write(data + b'\r\n// cf load acceptance\r\n')


def corpus(root):
    # One path per line; paths may hold spaces.
    baseline_path = os.path.join(HERE, '..', 'tests', 'fixtures', 'external', 'corpus-baseline.txt')
    baseline = set(line.strip() for line in open(baseline_path, encoding='utf-8') if line.strip())
    ib = os.path.join(root, 'work-ext-8.3.27.2214', 'ib-cfg')
    work = os.path.join(root, 'loadcheck-corpus')
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    ok = failed = 0
    for f in sorted(os.listdir(os.path.join(root, 'ext-bin'))):
        name, ext = os.path.splitext(f)
        base = os.path.join(root, 'ext-bin', f)
        tree = os.path.join(work, name, 'tree')
        code, out = run('cf', 'export', base, tree, '--source-version', '2.20')
        if code:
            print('EXPORT FAIL', name)
            failed += 1
            continue
        edited = []
        om = os.path.join(tree, name, 'Ext', 'ObjectModule.bsl')
        if os.path.exists(om):
            append_comment(om)
            edited.append(name + '/Ext/ObjectModule.bsl')
        forms = os.path.join(tree, name, 'Forms')
        if os.path.isdir(forms):
            for form in sorted(os.listdir(forms)):
                fm = os.path.join(forms, form, 'Ext', 'Form', 'Module.bsl')
                if os.path.exists(fm):
                    append_comment(fm)
                    edited.append('%s/Forms/%s/Ext/Form/Module.bsl' % (name, form))
                    break
        if not edited:
            print('skip (nothing to edit)', name)
            continue
        loaded = os.path.join(work, name, 'loaded' + ext)
        code, out = run('cf', 'load', tree, loaded, '--base', base)
        if code:
            print('LOAD FAIL', name, out[-300:].replace('\n', ' '))
            failed += 1
            continue
        dump = os.path.join(work, name, 'dump')
        try:
            v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                                   os.path.join(dump, name + '.xml'), loaded], os.path.join(work, 'log.txt'))
        except v8dump.DumpError as ex:
            print('PLATFORM REJECTED', name, str(ex)[-300:].replace('\n', ' '))
            failed += 1
            continue
        ours, nat = files(tree), files(dump)
        bad = [p for p in ours if (name + '/' + p in baseline or p in edited)
               and (p not in nat or open(nat[p], 'rb').read() != open(ours[p], 'rb').read())]
        if bad:
            print('MISMATCH', name, bad[:3])
            failed += 1
        else:
            ok += 1
    print('corpus: %d loaded and accepted by the platform, %d failed' % (ok, failed))


def recovered_source(sources, tree, bin_path):
    """The recovered text for the closed module `bin_path`, or None."""
    rel = os.path.relpath(bin_path[:-4] + '.bsl', tree)
    candidates = [os.path.join(sources, rel)]
    if os.path.basename(bin_path) == 'Module.bin':
        owner = os.path.basename(os.path.dirname(os.path.dirname(bin_path)))
        candidates.append(os.path.join(sources, owner + '.bsl'))
    return next((c for c in candidates if os.path.exists(c)), None)


def cf(path, sources):
    work = os.path.join(os.path.dirname(os.path.abspath(path)), 'loadcheck-cf')
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    tree = os.path.join(work, 'tree')
    code, out = run('cf', 'export', path, tree, '--source-version', '2.20')
    print('export exit', code)
    # 2: some entries failed and are listed in the report; the tree is written.
    if code not in (0, 2) or not os.path.isdir(tree):
        print('export failed:', out[-400:])
        return
    replaced = []
    for dp, _, fs in os.walk(tree):
        for f in fs:
            if not (f.endswith('Module.bin') and os.path.basename(dp) == 'Ext'):
                continue
            src = recovered_source(sources, tree, os.path.join(dp, f))
            if src is None:
                continue
            os.remove(os.path.join(dp, f))
            text = open(src, 'rb').read()
            if not text.startswith(BOM):
                text = BOM + text
            bsl = os.path.join(dp, f[:-4] + '.bsl')
            open(bsl, 'wb').write(text)
            replaced.append(os.path.relpath(bsl, tree).replace(os.sep, '/'))
    print('replaced closed modules:', len(replaced))
    loaded = os.path.join(work, 'loaded.cf')
    code, out = run('cf', 'load', tree, loaded, '--base', path)
    print('load exit', code, out[-400:])
    if code:
        return
    ib = os.path.join(work, 'ib')
    log = os.path.join(work, 'log.txt')
    v8dump._run(PLATFORM, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/LoadCfg', loaded], log)
    text = v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/CheckModules', '-Server', '-ThinClient'], log,
                       ok=(0, 1, 101))
    owners = [m.split('/Ext/')[0].split('/')[-1] for m in replaced]
    errs = [l for l in text.splitlines() if any(o in l for o in owners)]
    print('CheckModules records mentioning replaced modules:', len(errs))
    for l in errs[:10]:
        print('   ', l[:200])
    dump = os.path.join(work, 'dump')
    v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    norm = lambda b: b.replace(b'\r\n', b'\n').strip()
    for m in replaced:
        got_path = os.path.join(dump, m)
        got = open(got_path, 'rb').read() if os.path.exists(got_path) else b''
        want = open(os.path.join(tree, m), 'rb').read()
        print('%-60s dumped == recovered source: %s' % (m, norm(got) == norm(want)))


if __name__ == '__main__':
    if sys.argv[1] == 'corpus':
        corpus(sys.argv[2])
    else:
        cf(sys.argv[2], sys.argv[3])
