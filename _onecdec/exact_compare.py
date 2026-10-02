"""One build's export of a local corpus against the platform's dump: every
file that differs, fast enough for a 100 000-file configuration.

    python exact_compare.py <label> [--native DIR] [--out DIR] [--baseline DIR] [--no-export]

Exports corpora.local.json's <label> with IBCMD_EXE (below-normal priority,
RAYON_NUM_THREADS 12 unless set) into --out (default <IBCMD_WORK>/exact/<label>)
and compares it with --native (default <IBCMD_WORK>/native-8.3.27.2214/<label>):
sizes first, contents only where sizes agree, eight files at a time. With
--baseline (another build's export of the same file), each differing file is
also checked there: one the baseline matches is a regression.
"""
import os, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

sys.stdout.reconfigure(encoding='utf-8')
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import big_compare as bc
import local_paths


def arg(name, default=None):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


def walk(root):
    out = {}
    for dp, _, fs in os.walk(root):
        for f in fs:
            if f == '.complete':
                continue
            p = os.path.join(dp, f)
            out[os.path.relpath(p, root).replace(os.sep, '/')] = p
    return out


def same(a, b):
    try:
        if os.path.getsize(a) != os.path.getsize(b):
            return False
        with open(a, 'rb') as x, open(b, 'rb') as y:
            return x.read() == y.read()
    except OSError:
        return False


def export(label, out):
    env = dict(os.environ)
    env.setdefault('RAYON_NUM_THREADS', '12')
    flags = 0x00004000 if os.name == 'nt' else 0  # BELOW_NORMAL_PRIORITY_CLASS
    r = subprocess.run([local_paths.EXE, 'cf', 'export', bc.CORPORA[label], out, '--overwrite'],
                       capture_output=True, env=env, creationflags=flags)
    # The report beside the tree: what failed or was noted, when exit != 0.
    open(out.rstrip('\\/') + '.report.json', 'wb').write(r.stdout or r.stderr)
    return r.returncode


def groups(differ, nat, ours):
    """The differing files grouped by their first differing line pair."""
    import difflib, re
    counts, example = {}, {}
    for p in differ:
        a = open(nat[p], 'rb').read().decode('utf-8', 'replace').splitlines()
        b = open(ours[p], 'rb').read().decode('utf-8', 'replace').splitlines()
        lines = [l for l in difflib.unified_diff(a, b, lineterm='', n=0) if l[:1] in '+-' and l[:3] not in ('---', '+++')]
        key = ' | '.join(re.sub(r'>[^<]*<', '>…<', l.strip())[:90] for l in lines[:2])
        counts[key] = counts.get(key, 0) + 1
        example.setdefault(key, p)
    for key, n in sorted(counts.items(), key=lambda kv: -kv[1])[:40]:
        print('  x%-4d %s   e.g. %s' % (n, key, example[key]))


def main():
    label = [a for a in sys.argv[1:] if not a.startswith('--') and sys.argv[sys.argv.index(a) - 1] not in
             ('--native', '--out', '--baseline')][0]
    native = arg('--native', os.path.join(local_paths.WORK, 'native-8.3.27.2214', label))
    out = arg('--out', os.path.join(local_paths.WORK, 'exact', label))
    baseline = arg('--baseline')
    code = None if '--no-export' in sys.argv else export(label, out)
    nat, ours = walk(native), walk(out)
    common = sorted(p for p in nat if p in ours)
    with ThreadPoolExecutor(8) as pool:
        verdicts = list(pool.map(lambda p: same(nat[p], ours[p]), common))
    differ = [p for p, ok in zip(common, verdicts) if not ok]
    missing = sorted(p for p in nat if p not in ours)
    extra = sorted(p for p in ours if p not in nat)
    total = len(nat)
    identical = total - len(differ) - len(missing)
    print('%s exit=%s identical %d/%d (%.3f%%) differ %d missing %d extra %d' % (
        label, code, identical, total, 100.0 * identical / max(1, total), len(differ), len(missing), len(extra)))
    if '--groups' in sys.argv:
        groups(differ, nat, ours)
        by_dir = {}
        for p in missing:
            key = '/'.join(p.split('/')[:1]) + ' ' + p.rsplit('/', 1)[-1]
            by_dir[key] = by_dir.get(key, 0) + 1
        for key, n in sorted(by_dir.items(), key=lambda kv: -kv[1])[:15]:
            print('  missing x%d %s' % (n, key))
        return
    for kind, paths in (('differ', differ), ('missing', missing), ('extra', extra)):
        for p in paths[:60]:
            note = ''
            if baseline and kind != 'extra':
                b = os.path.join(baseline, p.replace('/', os.sep))
                note = '  REGRESSION (baseline matches)' if os.path.exists(b) and same(nat[p], b) else ''
            print('  %-7s %s%s' % (kind, p, note))
    sys.stdout.flush()


if __name__ == '__main__':
    main()
