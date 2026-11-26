"""Copied actual case4 negative fixtures; originals read-only, no DB/process calls."""
import hashlib
import importlib.util
from pathlib import Path
import re
import shutil
import sys
sys.dont_write_bytecode = True
import tempfile

spec=importlib.util.spec_from_file_location('case4',Path(__file__).with_name('analyze_private4.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
lab=Path(sys.argv[1]).resolve()
if not lab.is_relative_to(Path('F:/ibcmd/lab/05').resolve()): raise ValueError('F 0.5 fixture required')
inputs=[lab/'snapshots/private4-physical-documents.txt',*sorted((lab/'obs').glob('private4-load-4-*.log'))]
def fingerprint(): return {str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
before=fingerprint()
assert m.reconcile(lab)['confirmed_reports']==153
with tempfile.TemporaryDirectory(prefix='private4-readonly-negative-',dir=lab/'logs') as folder:
    fixture=Path(folder);(fixture/'snapshots').mkdir();(fixture/'obs').mkdir()
    def reset():
        for p in inputs: shutil.copyfile(p,fixture/p.relative_to(lab))
    for name in ('wrong-UUID','unposted','duplicate-physical','report10','report1extra','duplicate-report'):
        reset()
        p=fixture/('snapshots/private4-physical-documents.txt' if name in ('wrong-UUID','unposted','duplicate-physical') else 'obs/private4-load-4-old-a.log')
        original=p.read_text(encoding='utf-8-sig')
        if name=='wrong-UUID': changed=re.sub(r'(?m)^[A-F0-9]{32}\|','0'*32+'|',original,count=1)
        elif name=='unposted': changed=original.replace('|01|','|00|',1)
        elif name=='duplicate-physical': changed=original+next(r for r in original.splitlines() if 'ibcmd-rs-load:' in r)+'\n'
        elif name=='report10': changed=original.replace('report_ok=1;','report_ok=10;',1)
        elif name=='report1extra': changed=original.replace('report_ok=1;','report_ok=1extra;',1)
        else: changed=original.replace('report_ok=1;','report_ok=1;report_ok=1;',1)
        assert changed != original
        p.write_text(changed,encoding='utf-8')
        try: m.reconcile(fixture)
        except ValueError as error: print('PASS copied',name,'REFUSED:',str(error))
        else: raise AssertionError(name+' accepted')
    output=fixture/'immutable.json';m.publish(output,b'original\n');sha=m.digest(output)
    m.publish(output,b'original\n')
    try: m.publish(output,b'different\n')
    except ValueError: pass
    else: raise AssertionError('different output overwritten')
    assert m.digest(output)==sha
    print('PASS existing different output refused, SHA unchanged; identical retry accepted')
assert before==fingerprint()
print('PASS original physical/journal SHA unchanged; DB/process/native calls0')
