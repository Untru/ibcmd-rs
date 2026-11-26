"""Compare our export of the large local corpora with the platform's dumps.

    python big_compare.py [label...]

Inputs are the user's own files (never copied into the repo); native dumps come
from `oracle_dump.py 8.3.27.2214 <IBCMD_WORK> ...` (labels below). Writes our exports
to <IBCMD_WORK>\\ours\\<label> and prints per corpus: identical/total, missing, extra,
then the most frequent first-differing lines across all corpora.
"""
import collections, difflib, json, os, subprocess, sys

import local_paths  # noqa: E402
EXE = local_paths.EXE
ROOT = local_paths.WORK
NATIVE = os.path.join(ROOT, 'native-8.3.27.2214')
# label -> input file: the user's own data, listed in the git-ignored
# corpora.local.json next to this script ({"label": "path", ...}).
_LOCAL = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'corpora.local.json')
CORPORA = json.load(open(_LOCAL, encoding='utf-8')) if os.path.exists(_LOCAL) else {}
# Reference trees dumped earlier, in the corpus directory.
EARLIER = os.path.join(local_paths.CORPUS, 'native-8.3.27.2214')
NATIVE_OF = {'src': EARLIER + r'\src_1cv8', 'obf': EARLIER + r'\obf_1cv8', 'itk': EARLIER + r'\itk_ssl_ru'}


def native_of(label):
    return NATIVE_OF.get(label, os.path.join(NATIVE, label))


def files(root):
    out = {}
    for dp, _, fs in os.walk(root):
        for f in fs:
            if f == '.complete':
                continue
            p = os.path.join(dp, f)
            out[os.path.relpath(p, root).replace(os.sep, '/')] = p
    return out


def main():
    labels = sys.argv[1:] or [l for l in CORPORA if os.path.exists(os.path.join(native_of(l), '.complete'))]
    groups, example = collections.Counter(), {}
    for label in labels:
        out = os.path.join(os.environ.get('IBCMD_OURS', os.path.join(ROOT, 'ours')), label)
        r = subprocess.run([EXE, 'cf', 'export', CORPORA[label], out, '--source-version', '2.20', '--overwrite'],
                           capture_output=True)
        report = json.loads(r.stdout or r.stderr or b'{}')
        entries = (report.get('export') or {}).get('storage', {}).get('entries', [])
        noted = [e for e in entries if e.get('message') and e['logical_name'] not in ('root', 'version', 'versions')]
        nat, ours = files(native_of(label)), files(out)
        same = [p for p in nat if p in ours and open(nat[p], 'rb').read() == open(ours[p], 'rb').read()]
        missing = [p for p in nat if p not in ours]
        extra = [p for p in ours if p not in nat]
        print('%-5s exit=%d identical %d/%d (%.2f%%) missing %d extra %d noted %d' % (
            label, r.returncode, len(same), len(nat), 100.0 * len(same) / max(1, len(nat)),
            len(missing), len(extra), len(noted)))
        for e in noted[:8]:
            print('      note', e['logical_name'], e['disposition'], e['message'][:160])
        for p in nat:
            if p in missing:
                k = 'missing ' + p.rsplit('/', 1)[-1]
            elif p not in same:
                a = open(ours[p], encoding='utf-8-sig', errors='replace').read().splitlines()
                b = open(nat[p], encoding='utf-8-sig', errors='replace').read().splitlines()
                d = [l for l in difflib.unified_diff(a, b, lineterm='', n=0)
                     if l[:1] in '+-' and not l.startswith(('---', '+++'))]
                k = 'diff ' + (d[0].strip()[:110] if d else '?')
            else:
                continue
            groups[k] += 1
            example.setdefault(k, label + '/' + p)
        for p in extra:
            groups['extra ' + p.rsplit('/', 1)[-1]] += 1
            example.setdefault('extra ' + p.rsplit('/', 1)[-1], label + '/' + p)
        sys.stdout.flush()
    for k, v in groups.most_common(70):
        print('%5d %s   e.g. %s' % (v, k, example[k][:100]))


if __name__ == '__main__':
    main()
