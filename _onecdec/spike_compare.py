"""SPIKE (throwaway): run the adapter over corpus .epf files and compare the
non-root files with the native empty-infobase dump.

    python spike_compare.py <corpus-root> [names...]
"""
import os, sys, json, subprocess, shutil
import spike_adapter, v8c

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.path.join(HERE, '..', 'target', 'release', 'ibcmd-rs.exe')


def files(r):
    out = {}
    for dp, _, fs in os.walk(r):
        for f in fs:
            if f == '.complete': continue
            p = os.path.join(dp, f); out[os.path.relpath(p, r).replace(os.sep, '/')] = p
    return out


def one(corpus, name):
    epf = os.path.join(corpus, 'ext-bin', name + '.epf')
    if not os.path.exists(epf): return None
    work = os.path.join(corpus, 'spike', name); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    cf = os.path.join(work, 'x.cf')
    e = v8c.read(epf)
    main_id = e['root'].decode('utf-8-sig').split(',')[1].strip()
    import re
    cls, obj, dp = spike_adapter.adapt(e[main_id].decode('utf-8-sig'))
    out = {k: v for k, v in e.items() if k not in (main_id, 'copyinfo')}
    out[obj] = b'\xef\xbb\xbf' + dp.encode('utf-8')
    v = e['versions'].decode('utf-8-sig').replace('"%s"' % main_id, '"%s"' % obj)
    v = re.sub(r',\s*"copyinfo",\s*[0-9a-f-]{36}', '', v)
    n = len(re.findall(r'"[^"]*",\s*[0-9a-f-]{36}', v))
    v = re.sub(r'^\{1,\d+,', '{1,%d,' % n, v)
    out['versions'] = b'\xef\xbb\xbf' + v.encode('utf-8')
    open(cf, 'wb').write(v8c.write15(out))
    r = subprocess.run([EXE, 'cf', 'export', cf, os.path.join(work, 'out'), '--source-version', '2.20'],
                       capture_output=True)
    rep = json.loads((r.stdout or r.stderr).decode('utf-8'))
    bad = []
    def walk(x):
        if isinstance(x, dict):
            if 'disposition' in x and x['disposition'] != 'supported' and x['logical_name'] not in ('root', 'version', 'versions'):
                bad.append('%s %s %s' % (x['disposition'], x['logical_name'], x.get('message', '')[:160]))
            for vv in x.values(): walk(vv)
        elif isinstance(x, list):
            for vv in x: walk(vv)
    walk(rep)
    ours = files(os.path.join(work, 'out', 'DataProcessors', name)) if os.path.isdir(os.path.join(work, 'out', 'DataProcessors', name)) else {}
    pat = re.compile(r'(?<![\w.])DataProcessor(Object|TabularSection|TabularSectionRow)?\.%s(?![\w])' % re.escape(name))
    for p in ours.values():
        if p.endswith(('.xml', '.html')):
            b = open(p, 'rb').read(); t = b.decode('utf-8')
            t2 = pat.sub(lambda m: 'ExternalDataProcessor%s.%s' % (m.group(1) or '', name), t)
            if t2 != t: open(p, 'wb').write(t2.encode('utf-8'))
    nat = files(os.path.join(corpus, 'native-8.3.27.2214', 'ext', name, name))
    same = [p for p in nat if p in ours and open(nat[p], 'rb').read() == open(ours[p], 'rb').read()]
    diff = [p for p in nat if p in ours and p not in same]
    miss = [p for p in nat if p not in ours]
    extra = [p for p in ours if p not in nat]
    return dict(name=name, native=len(nat), same=len(same), diff=diff, miss=miss, extra=extra, bad=bad)


def main():
    corpus = sys.argv[1]
    names = sys.argv[2:] or sorted(f[:-4] for f in os.listdir(os.path.join(corpus, 'ext-bin')) if f.endswith('.epf'))
    tot = dict(native=0, same=0)
    for n in names:
        r = one(corpus, n)
        if r is None: continue
        tot['native'] += r['native']; tot['same'] += r['same']
        print('%-55s %3d/%3d same  diff=%d miss=%d extra=%d bad=%d' % (n[:55], r['same'], r['native'], len(r['diff']), len(r['miss']), len(r['extra']), len(r['bad'])))
        for k in ('diff', 'miss', 'extra', 'bad'):
            for p in r[k][:4]: print('     %-5s %s' % (k, p))
    print('TOTAL non-root files identical: %d/%d' % (tot['same'], tot['native']))


if __name__ == '__main__':
    main()
