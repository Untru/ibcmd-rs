"""`cf load` of one Form.xml edit at a time: the first `<v8:content>` of each
form (or of every n-th, --step n) gets ` (правка)`, the tree loads onto the
file, the file is restored. Counts forms that load (the load's own check
passed) and those refused, with the first line of the reason.

    python stress_forms.py <file or label:<name>> [--step N]

The binary is IBCMD_EXE or target-main's build.
"""
import json, os, re, shutil, subprocess, sys, tempfile
sys.stdout.reconfigure(encoding='utf-8')

import local_paths  # noqa: E402
EXE = local_paths.EXE
HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    target = sys.argv[1]
    if target.startswith('label:'):
        target = json.load(open(os.path.join(HERE, 'corpora.local.json'), encoding='utf-8'))[target[6:]]
    step = int(sys.argv[sys.argv.index('--step') + 1]) if '--step' in sys.argv else 1
    work = tempfile.mkdtemp(prefix='ibcmd-forms-')
    tree = os.path.join(work, 'tree')
    subprocess.run([EXE, 'cf', 'export', target, tree, '--overwrite'], capture_output=True)
    forms = sorted(os.path.join(dp, f) for dp, _, fs in os.walk(tree) for f in fs if f == 'Form.xml')
    tally, reasons = {'ok': 0, 'refused': 0, 'skipped': 0}, {}
    try:
        for form in forms[::step]:
            data = open(form, 'rb').read()
            m = re.search(rb'<v8:content>([^<]+)</v8:content>', data)
            if not m:
                tally['skipped'] += 1
                continue
            open(form, 'wb').write(data[:m.end(1)] + ' (правка)'.encode() + data[m.end(1):])
            out = os.path.join(work, 'out' + os.path.splitext(target)[1])
            if os.path.exists(out):
                os.remove(out)
            r = subprocess.run([EXE, 'cf', 'load', tree, out, '--base', target], capture_output=True)
            open(form, 'wb').write(data)
            rel = os.path.relpath(form, tree)
            if r.returncode == 0:
                tally['ok'] += 1
                print('ok      ', rel, flush=True)
            else:
                tally['refused'] += 1
                try:
                    message = json.loads(r.stderr or r.stdout)['errors'][0]['message']
                except Exception:
                    message = (r.stderr or r.stdout).decode('utf-8', 'replace')
                reason = re.sub(r'C:\\\S*', '', message)[:160]
                key = reason[:80]
                reasons[key] = reasons.get(key, 0) + 1
                print('refused ', rel, '|', reason, flush=True)
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print('total', tally)
    for reason, count in sorted(reasons.items(), key=lambda item: -item[1])[:15]:
        print('%4d  %s' % (count, reason))


if __name__ == '__main__':
    main()
