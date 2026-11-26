"""Compare `ibcmd-rs cf export` of every corpus .epf/.erf with the platform's
dump made in the configuration infobase (native-<ver>/ext-cfg/<X>).

    python corpus_compare.py <corpus-root> [--ver 8.3.27.2214] [--show N] [names...]

Prints per-object stats, then the diff groups (first differing line pair per
file, bucketed) so fixes can be prioritised.
"""
import collections, difflib, json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.path.join(HERE, '..', 'target', 'release', 'ibcmd-rs.exe')


def files(r):
    out = {}
    for dp, _, fs in os.walk(r):
        for f in fs:
            if f == '.complete': continue
            p = os.path.join(dp, f)
            out[os.path.relpath(p, r).replace(os.sep, '/')] = p
    return out


def first_diff(a, b):
    la = open(a, 'rb').read().decode('utf-8', 'replace').splitlines()
    lb = open(b, 'rb').read().decode('utf-8', 'replace').splitlines()
    for x in difflib.unified_diff(la, lb, lineterm='', n=0):
        if x.startswith(('---', '+++', '@@')): continue
        return x.strip()[:160]
    return '(whitespace/eol only)'


def main():
    a = sys.argv[1:]
    corpus = a[0]
    ver = a[a.index('--ver') + 1] if '--ver' in a else '8.3.27.2214'
    show = int(a[a.index('--show') + 1]) if '--show' in a else 3
    names = [x for x in a[1:] if not x.startswith('--') and x != ver and not x.isdigit()]
    binroot = os.path.join(corpus, 'ext-bin')
    items = sorted(f for f in os.listdir(binroot) if f.endswith(('.epf', '.erf')))
    if names: items = [f for f in items if f[:-4] in names]
    tot_n = tot_same = 0
    groups = collections.defaultdict(list)
    failed_exports = []
    for f in items:
        name = f[:-4]
        out = os.path.join(corpus, 'cmp', name)
        r = subprocess.run([EXE, 'cf', 'export', os.path.join(binroot, f), out, '--source-version', '2.20', '--overwrite'],
                           capture_output=True)
        if r.returncode != 0:
            try:
                rep = json.loads((r.stderr or r.stdout).decode('utf-8'))
                msgs = []
                def walk(x):
                    if isinstance(x, dict):
                        if x.get('disposition') == 'failed': msgs.append(x.get('message', '')[:140])
                        for v in x.values(): walk(v)
                    elif isinstance(x, list):
                        for v in x: walk(v)
                walk(rep)
                if not msgs: msgs = [e.get('message', '')[:200] for e in rep.get('errors', [])]
            except Exception:
                msgs = [(r.stderr or r.stdout).decode('utf-8', 'replace')[:200]]
            failed_exports.append((name, msgs))
        nat = files(os.path.join(corpus, 'native-' + ver, 'ext-cfg', name))
        ours = files(out)
        same = [p for p in nat if p in ours and open(nat[p], 'rb').read() == open(ours[p], 'rb').read()]
        tot_n += len(nat); tot_same += len(same)
        for p in nat:
            if p not in ours:
                groups['missing ' + os.path.splitext(p)[1] + ' ' + p.split('/')[-1] if p.count('/') < 3 else 'missing ' + '/'.join(p.split('/')[-2:])].append(name + '/' + p)
            elif p not in same:
                groups['diff ' + first_diff(ours[p], nat[p])].append(name + '/' + p)
        for p in ours:
            if p not in nat: groups['extra ' + p.split('/')[-1]].append(name + '/' + p)
    print('TOTAL identical files: %d/%d (%.1f%%), objects: %d, exports failing: %d'
          % (tot_same, tot_n, 100.0 * tot_same / max(1, tot_n), len(items), len(failed_exports)))
    for name, msgs in failed_exports[:20]:
        print('  FAILED EXPORT %s: %s' % (name, ' | '.join(msgs[:2])))
    for k, v in sorted(groups.items(), key=lambda kv: -len(kv[1]))[:40]:
        print('%4d  %s' % (len(v), k))
        for p in v[:show]: print('        ' + p)


if __name__ == '__main__':
    main()
