"""`cf load` over many files: each is exported, edited the same way, loaded
onto itself, and exported back; the back export must equal the edited tree.

    python stress_load.py <file or directory>... [--platform] [--limit N]

Edits, where the tree has them:
  root     `<Comment/>` of the first object XML -> `<Comment>правка</Comment>`
  form     the first `<v8:content>` of the first Form.xml gets ` (правка)`
  module   the first module text gets a trailing comment line
Outcome per file: `ok`, `export-failed`, `load-failed <message>`, or
`differ <files>`. --platform also loads the result with 8.3.27.2214
on fresh disposable bases and compares native dumps. The binary is
IBCMD_EXE or target-main's build.
"""
import json, os, re, shutil, subprocess, sys, tempfile
sys.stdout.reconfigure(encoding='utf-8')

import local_paths  # noqa: E402
EXE = local_paths.EXE
HERE = os.path.dirname(os.path.abspath(__file__))
VERSION = re.compile(rb' configVersion="[^"]*"')
KINDS = ('.cf', '.cfe', '.epf', '.erf')


def run(args):
    return subprocess.run([EXE] + args, capture_output=True)


def files(root):
    out = {}
    for dp, dirs, fs in os.walk(root):
        dirs[:] = [d for d in dirs if not d.startswith('.')]
        for f in fs:
            p = os.path.join(dp, f)
            rel = os.path.relpath(p, root).replace(os.sep, '/')
            data = open(p, 'rb').read()
            if rel == 'ConfigDumpInfo.xml':
                data = VERSION.sub(b'', data)
            out[rel] = data
    return out


def replace_first(path, old, new):
    data = open(path, 'rb').read()
    if old not in data:
        return False
    open(path, 'wb').write(data.replace(old, new, 1))
    return True


def edit(tree, selected=None):
    done = []
    names = sorted(files(tree))
    if selected is not None:
        names = [name for name in names if name in selected]
    roots = [n for n in names if n.endswith('.xml') and '/Ext/' not in n and not n.startswith('Ext/')
             and n not in ('Configuration.xml', 'ConfigDumpInfo.xml') and n.count('/') <= 1]
    for n in roots:
        if replace_first(os.path.join(tree, n), b'<Comment/>', '<Comment>правка</Comment>'.encode()):
            done.append(n)
            break
    for n in names:
        if n.endswith('/Ext/Form.xml'):
            p = os.path.join(tree, n)
            data = open(p, 'rb').read()
            m = re.search(rb'<v8:content>([^<]+)</v8:content>', data)
            if m:
                open(p, 'wb').write(data[:m.end(1)] + ' (правка)'.encode() + data[m.end(1):])
                done.append(n)
                break
    for n in names:
        if n.endswith('Module.bsl'):
            with open(os.path.join(tree, n), 'ab') as f:
                f.write('\r\n// правка\r\n'.encode())
            done.append(n)
            break
    return done


def platform_dump(path, dump, work):
    """The platform's dump of an external object (8.3.27.2214), laid out as
    this exporter lays it out (root file named after the object)."""
    import local_paths
    sys.path.insert(0, local_paths.ONECDEC_TOOLS)
    import v8dump
    exe = os.path.join('C:' + os.sep, 'Program Files', '1cv8', '8.3.27.2214', 'bin', '1cv8.exe')
    ib = os.path.join(work, 'ib-' + os.path.basename(dump))
    log = os.path.join(work, 'platform.log')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    os.makedirs(dump)
    if os.path.splitext(path)[1].lower() in ('.cf', '.cfe'):
        extension = ['-Extension', 'CorpusCheck'] if path.lower().endswith('.cfe') else []
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadCfg', path] + extension, log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump] + extension, log)
        return dump
    staging = os.path.join(work, 'staging-' + os.path.basename(dump))
    os.makedirs(staging)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                      os.path.join(staging, 'x.xml'), path], log)
    # The dump names the root after the file; take the object's name from
    # the root element's <Name>.
    root = open(os.path.join(staging, 'x.xml'), 'rb').read()
    name = re.search(rb'<Name>([^<]+)</Name>', root).group(1).decode('utf-8')
    shutil.move(os.path.join(staging, 'x.xml'), os.path.join(dump, name + '.xml'))
    if os.path.isdir(os.path.join(staging, 'x')):
        shutil.move(os.path.join(staging, 'x'), os.path.join(dump, name))
    return dump


def one(path, platform):
    work = tempfile.mkdtemp(prefix='ibcmd-stress-')
    try:
        tree, back = os.path.join(work, 'tree'), os.path.join(work, 'back')
        out = os.path.join(work, 'out' + os.path.splitext(path)[1])
        if run(['cf', 'export', path, tree, '--overwrite']).returncode not in (0, 2):
            return 'export-failed', []
        edits = edit(tree)
        r = run(['cf', 'load', tree, out, '--base', path])
        if r.returncode != 0:
            with open(os.path.join(work, 'load-report.json'), 'wb') as report:
                report.write(r.stderr or r.stdout)
            try:
                errors = json.loads(r.stderr or r.stdout)['errors']
                message = errors[0]['message'] if errors else '?'
            except Exception:
                message = (r.stderr or r.stdout)[-300:].decode('utf-8', 'replace')
            return 'load-failed ' + message[:300], edits
        exported = run(['cf', 'export', out, back, '--overwrite'])
        if exported.returncode not in (0, 2):
            return 'export-back-failed ' + (exported.stderr or exported.stdout)[-300:].decode('utf-8', 'replace'), edits
        want, got = files(tree), files(back)
        want.pop('ConfigDumpInfo.xml', None); got.pop('ConfigDumpInfo.xml', None)
        differ = sorted(p for p in set(want) | set(got) if want.get(p) != got.get(p))
        if differ:
            return 'differ ' + ', '.join(differ[:5]), edits
        if platform:
            # The platform's own dump of the base, edited the same way, must be
            # the platform's dump of the loaded file: independent of how well
            # this exporter reproduces the platform.
            expected = platform_dump(path, os.path.join(work, 'native'), work)
            got = platform_dump(out, os.path.join(work, 'loaded'), work)
            native_edits = edit(expected, selected=edits)
            if native_edits != edits:
                return 'platform-edit-mismatch ' + ', '.join(native_edits), edits
            want, have = files(expected), files(got)
            want.pop('ConfigDumpInfo.xml', None); have.pop('ConfigDumpInfo.xml', None)
            # The platform regenerates the integration-service identity on
            # each load. Keep the same documented normalization as verify_edit.
            from verify_edit import INTEGRATION
            for mapping in (want, have):
                if 'Configuration.xml' in mapping:
                    mapping['Configuration.xml'] = INTEGRATION.sub(lambda m: m.group(1), mapping['Configuration.xml'])
            differ = sorted(p for p in set(want) | set(have) if want.get(p) != have.get(p))
            if differ:
                return 'platform ' + ', '.join(differ[:5]), edits
        return 'ok', edits
    finally:
        if '--keep-work' in sys.argv:
            print('work ' + work, flush=True)
        else:
            shutil.rmtree(work, ignore_errors=True)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    limit = int(sys.argv[sys.argv.index('--limit') + 1]) if '--limit' in sys.argv else None
    if limit is not None:
        args = [a for a in args if a != str(limit)]
    inputs = []
    # `label:<name>` names a corpus of the git-ignored corpora.local.json.
    local = os.path.join(HERE, 'corpora.local.json')
    corpora = json.load(open(local, encoding='utf-8')) if os.path.exists(local) else {}
    args = [corpora[a[6:]] if a.startswith('label:') else a for a in args]
    for a in args:
        if os.path.isdir(a):
            inputs += sorted(os.path.join(a, f) for f in os.listdir(a) if f.lower().endswith(KINDS))
        else:
            inputs.append(a)
    inputs = inputs[:limit] if limit else inputs
    tally = {}
    for path in inputs:
        outcome, edits = one(path, '--platform' in sys.argv)
        key = outcome.split(' ')[0]
        tally[key] = tally.get(key, 0) + 1
        print('%-10s %s  [%s]%s' % (key, os.path.basename(path), len(edits),
                                     '' if key == 'ok' else '  ' + outcome[len(key):].strip()), flush=True)
    print('total', len(inputs), tally)


if __name__ == '__main__':
    main()
