"""Platform acceptance of `cf load` into a real extension: closed modules of
an exported .cfe are replaced by decompiled source, loaded offline, then the
platform loads the extension, checks the modules and dumps them back.

    python verify_load_cfe.py <base.cfe> <exported-tree> <decompiled-tree> <work>
"""
import os, re, shutil, subprocess, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

import local_paths  # noqa: E402
EXE = local_paths.EXE
PLATFORM = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'

base, tree, decompiled, work = sys.argv[1:5]
os.makedirs(work, exist_ok=True)
replaced = []
for dp, _, fs in os.walk(tree):
    if 'Module.bin' not in fs: continue
    rel = os.path.relpath(dp, tree)
    src = os.path.join(decompiled, rel, 'Module.bsl')
    if os.path.exists(src):
        os.remove(os.path.join(dp, 'Module.bin'))
        text = open(src, 'rb').read()
        if not text.startswith(b'\xef\xbb\xbf'): text = b'\xef\xbb\xbf' + text
        open(os.path.join(dp, 'Module.bsl'), 'wb').write(text)
        replaced.append(rel.replace(os.sep, '/'))
print('replaced:', replaced)
loaded = os.path.join(work, 'loaded.cfe')
if os.path.exists(loaded): os.remove(loaded)
r = subprocess.run([EXE, 'cf', 'load', tree, loaded, '--base', base], capture_output=True)
print('cf load exit', r.returncode, (r.stdout or r.stderr).decode('utf-8', 'replace')[-300:].replace('\n', ' '))
if r.returncode: sys.exit(1)
name = re.search(r'<Name>([^<]+)</Name>', open(os.path.join(tree, 'Configuration.xml'), encoding='utf-8-sig').read()).group(1)
ib = os.path.join(work, 'ib'); shutil.rmtree(ib, ignore_errors=True); log = os.path.join(work, 'log.txt')
v8dump._run(PLATFORM, ['CREATEINFOBASE', 'File="%s"' % ib], log)
v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/LoadCfg', loaded, '-Extension', name], log)
text = v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/CheckModules', '-Server', '-ThinClient', '-Extension', name],
                   log, ok=(0, 1, 101))
mods = [p.split('/')[1] for p in replaced]
errs = [l for l in text.splitlines() if any(m in l for m in mods)]
print('CheckModules lines mentioning replaced modules:', len(errs))
for l in errs[:8]: print('   ', l[:220])
dump = os.path.join(work, 'dump'); shutil.rmtree(dump, ignore_errors=True)
v8dump._run(PLATFORM, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', name], log)
norm = lambda b: b.replace(b'\r\n', b'\n').strip()
for rel in replaced:
    got = open(os.path.join(dump, rel, 'Module.bsl'), 'rb').read()
    want = open(os.path.join(tree, rel, 'Module.bsl'), 'rb').read()
    print('%-60s dumped == decompiled source: %s' % (rel, norm(got) == norm(want)))
