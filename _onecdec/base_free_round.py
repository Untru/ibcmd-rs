"""`.cf -> XML -> cf bootstrap --base-free -> CF -> XML`: the second export
must equal the first file for file (Untru/ibcmd-rs#351).

    python base_free_round.py <file.cf or directory>... [--keep DIR]

Prints per input: `ok`, `export-failed`, `bootstrap-failed <message>`, or
`differ <n> <first files>`. IBCMD_EXE names the build (local_paths.py).
"""
import json, os, re, shutil, subprocess, sys, tempfile

sys.stdout.reconfigure(encoding='utf-8')
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import local_paths

VERSION = re.compile(rb' configVersion="[^"]*"')


def run(args):
    return subprocess.run([local_paths.EXE] + args, capture_output=True)


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


def one(path, work):
    tree, back, out = (os.path.join(work, n) for n in ('tree', 'back', 'out.cf'))
    if run(['cf', 'export', path, tree, '--overwrite']).returncode not in (0, 2):
        return 'export-failed'
    r = run(['cf', 'bootstrap', tree, out, '--base-free'])
    if r.returncode != 0:
        try:
            message = json.loads(r.stdout or r.stderr)['errors'][0]['message']
        except Exception:
            message = (r.stderr or r.stdout)[-400:].decode('utf-8', 'replace')
        return 'bootstrap-failed ' + ' '.join(message.split())[:400]
    run(['cf', 'export', out, back, '--overwrite'])
    want, got = files(tree), files(back)
    differ = sorted(p for p in set(want) | set(got) if want.get(p) != got.get(p))
    return 'differ %d %s' % (len(differ), ', '.join(differ[:4])) if differ else 'ok'


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    keep = sys.argv[sys.argv.index('--keep') + 1] if '--keep' in sys.argv else None
    args = [a for a in args if a != keep]
    inputs = []
    for a in args:
        if os.path.isdir(a):
            for dp, _, fs in os.walk(a):
                inputs += [os.path.join(dp, f) for f in fs if f.lower().endswith('.cf')]
        else:
            inputs.append(a)
    tally = {}
    for path in sorted(inputs):
        work = keep or tempfile.mkdtemp(prefix='ibcmd-bf-')
        try:
            outcome = one(path, work)
        finally:
            if not keep:
                shutil.rmtree(work, ignore_errors=True)
        key = outcome.split(' ')[0]
        tally[key] = tally.get(key, 0) + 1
        print('%-17s %s  %s' % (key, os.path.relpath(path), outcome[len(key):].strip()[:300]), flush=True)
    print('total', len(inputs), tally)


if __name__ == '__main__':
    main()
