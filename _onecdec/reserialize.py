"""Load a native XML dump into 8.3.27 and save the .cf: the configuration
re-serialized in the current root tuple shape."""
import os, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
src, dst = sys.argv[1], sys.argv[2]
work = dst + '.work'; shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
ib = os.path.join(work, 'ib'); log = os.path.join(work, 'log')
v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', dst], log)
shutil.rmtree(work, ignore_errors=True)
print('ok', dst)
