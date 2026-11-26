"""Verify an offline-built .cf with the platform: LoadCfg -> CheckModules -> partial dump."""
import os, sys, shutil, time
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

S = sys.argv[1]; cf = sys.argv[2]; off = sys.argv[3]
mods = ['узОбщийМодульСервер', 'узРаботаССодержаниемЗадач', 'узТелеграмБот']
exe = v8dump.find_platform(); print('platform', exe)
work = os.path.join(S, 'verify'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
ib = os.path.join(work, 'ib'); log = os.path.join(work, 'v.log'); dump = os.path.join(work, 'dump')
t0 = time.time()
v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadCfg', cf], log)
print('LoadCfg ok %.0fs' % (time.time() - t0))
errs = v8dump._check(exe, ib, [], log)
mine = [e for e in errs if any(m in e for m in mods)]
print('CheckModules: %d error records total, %d in the rebuilt modules' % (len(errs), len(mine)))
for e in mine[:10]: print('   ', e[:300])
lst = os.path.join(work, 'list.txt')
open(lst, 'w', encoding='utf-8-sig').write('\n'.join('CommonModule.' + m for m in mods))
v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-listFile', lst], log)
for m in mods:
    p = os.path.join(dump, 'CommonModules', m, 'Ext')
    got = sorted(os.listdir(p)) if os.path.isdir(p) else []
    same = None
    if 'Module.bsl' in got:
        a = open(os.path.join(p, 'Module.bsl'), encoding='utf-8-sig').read().replace('\r\n', '\n')
        b = open(os.path.join(off, 'CommonModules', m, 'Ext', 'Module.bsl'), encoding='utf-8-sig').read().replace('\r\n', '\n')
        same = a.strip() == b.strip()
    print('%-28s platform dump: %s  text == decompiled: %s' % (m, got, same))
print('total %.0fs' % (time.time() - t0))
