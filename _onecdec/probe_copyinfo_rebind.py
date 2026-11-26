"""PROBE (throwaway): does the platform rebind an external object's
configuration references by the names in copyinfo when the stored uuids are
not the configuration's? Replaces every configuration object/type uuid named in
copyinfo with a fresh one (consistently across all entries), rebuilds the
container and dumps it in the configuration infobase.

    python probe_copyinfo_rebind.py <corpus-root> <name> <ext>
"""
import os, re, shutil, sys, uuid
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump
import v8c

corpus, name, ext = sys.argv[1:4]
src = os.path.join(corpus, 'ext-bin', name + '.' + ext)
e = v8c.read(src)
ci = e['copyinfo'].decode('utf-8-sig')
main = e['root'].decode('utf-8-sig').split(',')[1].strip()
own = set(re.findall(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', ''.join(
    k for k in e if k not in ('copyinfo', 'root', 'version', 'versions'))))
objects = re.findall(r'\{([0-9a-f-]{36}),\1,\d+,', ci)
types = re.findall(r'\{([0-9a-f-]{36}),[0-9a-f-]{36},\d+\}', ci)
foreign = [u for u in set(objects + types) if u not in own and u != main]
mapping = {u: str(uuid.uuid4()) for u in foreign}
print('replacing', len(mapping), 'configuration uuids')


def swap(b):
    try:
        t = b.decode('utf-8')
    except UnicodeDecodeError:
        return b
    for a, z in mapping.items():
        t = t.replace(a, z)
    return t.encode('utf-8')


out = {}
for k, v in e.items():
    if v[:4] == v8c.W15:           # nested container (module): rewrite its elements
        inner = v8c.V8(v, primary=False)
        els = {n: swap(inner.body(d)) for n, d in inner.entries()}
        out[k] = v8c.write15(els)
    else:
        out[k] = swap(v)
dst = os.path.join(corpus, 'probe', name + '.' + ext)
os.makedirs(os.path.dirname(dst), exist_ok=True)
open(dst, 'wb').write(v8c.write15(out))

exe = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
ib = os.path.join(corpus, 'work-ext-8.3.27.2214', 'ib-cfg')
dump = os.path.join(corpus, 'probe', 'dump-' + name)
shutil.rmtree(dump, ignore_errors=True); os.makedirs(dump)
v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                  os.path.join(dump, name + '.xml'), dst], os.path.join(corpus, 'probe', 'log.txt'))
native = os.path.join(corpus, 'native-8.3.27.2214', 'ext-cfg', name)
same = diff = 0
for dp, _, fs in os.walk(native):
    for f in fs:
        if f == '.complete': continue
        p = os.path.join(dp, f); q = os.path.join(dump, os.path.relpath(p, native))
        if os.path.exists(q) and open(p, 'rb').read() == open(q, 'rb').read(): same += 1
        else:
            diff += 1; print('DIFF', os.path.relpath(p, native))
print('identical to native config-infobase dump: %d/%d' % (same, same + diff))
