"""Load round trip against the platform: native XML -> `cf bootstrap
--base-free` -> 8.3.27.2214 loads the CF and dumps it -> compare with the
native XML it was built from.

    python load_compare.py <label> [<label> ...]

<label> names a native dump under <work>/native-8.3.27.2214/<label> (made by
oracle_dump.py). The built CF goes to <work>/load/<label>.cf, the platform's
dump of it to <work>/native-8.3.27.2214/load-<label>. Prints per label:
`build-failed <message>`, `dump-failed`, or `identical N/M differ D missing
X extra Y` with the first differing files.
"""
import os, re, shutil, subprocess, sys

sys.stdout.reconfigure(encoding='utf-8')
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import local_paths

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get('IBCMD_WORK') or local_paths.WORK
NATIVE = os.path.join(WORK, 'native-8.3.27.2214')
VERSION = re.compile(rb' configVersion="[^"]*"')


def files(root):
    out = {}
    for dp, dirs, fs in os.walk(root):
        for f in fs:
            p = os.path.join(dp, f)
            out[os.path.relpath(p, root).replace(os.sep, '/')] = p
    return out


def main():
    os.makedirs(os.path.join(WORK, 'load'), exist_ok=True)
    for label in sys.argv[1:]:
        source = os.path.join(NATIVE, label)
        cf = os.path.join(WORK, 'load', label + '.cf')
        if os.path.exists(cf):
            os.remove(cf)
        r = subprocess.run([local_paths.EXE, 'cf', 'bootstrap', '--base-free', '--platform',
                            '8.3.27.2214', source, cf], capture_output=True)
        if r.returncode != 0 or not os.path.exists(cf):
            msg = (r.stderr or r.stdout).decode('utf-8', 'replace').strip().splitlines()
            print(label, 'build-failed', ' | '.join(msg[-3:])[:600])
            continue
        dump_label = 'load-' + label
        shutil.rmtree(os.path.join(NATIVE, dump_label), ignore_errors=True)
        r = subprocess.run([sys.executable, os.path.join(HERE, 'oracle_dump.py'), '8.3.27.2214',
                            WORK, cf + '=' + dump_label], capture_output=True)
        dumped = os.path.join(NATIVE, dump_label)
        if not os.path.isdir(dumped):
            print(label, 'dump-failed', (r.stdout or r.stderr).decode('utf-8', 'replace')[-400:])
            continue
        a, b = files(source), files(dumped)
        # ConfigDumpInfo.xml carries the version stamps a load issues; the
        # platform's own XML load renumbers every one of them too (an isl
        # dump loaded back by 8.3.27.2214 changes all 4959), so the file is
        # compared with them removed.
        same, differ = 0, []
        for rel in sorted(a):
            if rel in b:
                x, y = open(a[rel], 'rb').read(), open(b[rel], 'rb').read()
                if rel == 'ConfigDumpInfo.xml':
                    x, y = VERSION.sub(b'', x), VERSION.sub(b'', y)
                if x == y:
                    same += 1
                else:
                    differ.append(rel)
        missing = [rel for rel in a if rel not in b]
        extra = [rel for rel in b if rel not in a]
        print(label, 'identical %d/%d (%.3f%%) differ %d missing %d extra %d' % (
            same, len(a), 100.0 * same / max(len(a), 1), len(differ), len(missing), len(extra)))
        for rel in (differ[:10] + ['missing ' + m for m in missing[:5]] + ['extra ' + e for e in extra[:5]]):
            print('   ', rel)


if __name__ == '__main__':
    main()
