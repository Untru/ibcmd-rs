"""Native oracle: dump every corpus file with an installed 1C platform.

    python oracle_dump.py <platform-version> <out-root> <file>[=<label>]...

Writes <out-root>/native-<version>/<stem>/ (Designer XML, hierarchical) plus
<stem>.log with the platform log; an existing complete dump is kept.
Used only to capture reference trees — the tool under test never runs 1C.
"""
import os, re, sys, shutil, time
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump


def exe_for(ver):
    p = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    if not os.path.isfile(p): raise SystemExit('no platform ' + p)
    return p


def dump_one(exe, src, out, work):
    kind = v8dump.KINDS[os.path.splitext(src)[1].lower()]
    log = out + '.log'
    shutil.rmtree(out, ignore_errors=True); os.makedirs(out)
    ib = os.path.join(work, 'ib'); shutil.rmtree(ib, ignore_errors=True)
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    if kind == 'ext':
        xml = os.path.join(out, os.path.splitext(os.path.basename(src))[0] + '.xml')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles', xml, src], log)
    else:
        ext = []
        if kind == 'cfe':
            ext = ['-Extension', re.sub(r'[^\wЁё]', '_', os.path.splitext(os.path.basename(src))[0])]
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadCfg', src] + ext, log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', out] + ext, log)
    shutil.rmtree(ib, ignore_errors=True)
    open(os.path.join(out, '.complete'), 'w').close()


def main():
    ver, root, files = sys.argv[1], sys.argv[2], sys.argv[3:]
    exe = exe_for(ver)
    base = os.path.join(root, 'native-' + ver); os.makedirs(base, exist_ok=True)
    work = os.path.join(root, 'work-' + ver); os.makedirs(work, exist_ok=True)
    for spec in files:
        f, _, label = spec.partition('=')          # file[=label]: label names the dump folder
        stem = label or os.path.splitext(os.path.basename(f))[0]
        out = os.path.join(base, stem)
        if os.path.exists(os.path.join(out, '.complete')):
            print('keep   %s' % out); continue
        t = time.time()
        try:
            dump_one(exe, os.path.abspath(f), out, work)
            n = sum(len(fs) for _, _, fs in os.walk(out)) - 1
            print('ok     %-45s %6d files %5.0fs' % (stem, n, time.time() - t))
        except v8dump.DumpError as ex:
            print('FAIL   %-45s %s' % (stem, str(ex)[:300]))
        sys.stdout.flush()


if __name__ == '__main__':
    main()
