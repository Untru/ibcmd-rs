"""big_compare.py for dialect 2.21: our `cf export --source-version 2.21` of
the local corpora against platform 8.5.1.1529 dumps.

    python big_compare_v85.py [label...]      # default: itk

Native dumps: <IBCMD_WORK>\\native-8.5\\<label> (`oracle_dump.dump_one` with
8.5.1.1529 in a unique work dir). Our exports: $IBCMD_OURS or
<IBCMD_WORK>\\ours-v85, per label. IBCMD_EXE overrides this checkout's target\\release build.
"""
import collections, difflib, json, os, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import big_compare as bc

import local_paths  # noqa: E402
NATIVE = os.path.join(local_paths.WORK, 'native-8.5')
# This checkout's build unless IBCMD_EXE names another.
EXE = os.environ.get('IBCMD_EXE') or os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'target', 'release', 'ibcmd-rs.exe')


def main():
    labels = sys.argv[1:] or ['itk']
    groups, example = collections.Counter(), {}
    for label in labels:
        out = os.path.join(os.environ.get('IBCMD_OURS', os.path.join(local_paths.WORK, 'ours-v85')), label)
        r = subprocess.run([EXE, 'cf', 'export', bc.CORPORA[label], out, '--source-version', '2.21', '--overwrite'],
                           capture_output=True)
        report = json.loads(r.stdout or r.stderr or b'{}')
        entries = (report.get('export') or {}).get('storage', {}).get('entries', [])
        noted = [e for e in entries if e.get('message') and e['logical_name'] not in ('root', 'version', 'versions')]
        nat, ours = bc.files(os.path.join(NATIVE, label)), bc.files(out)
        nat = {k: v for k, v in nat.items() if not k.endswith('.log')}
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
