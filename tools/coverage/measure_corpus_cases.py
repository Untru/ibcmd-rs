#!/usr/bin/env python3
"""Measure generated corpus cases in both portable directions, without SQL.

CF export is checked against every native byte (except ConfigDumpInfo, which
the coverage protocol excludes). audit-empty-stage measures only compilation
of XML into rows; zero failures is NOT proof of SQL import/apply parity.
An empty rows folder is deliberately used, so 'extra' rows are expected.
"""
import argparse,base64,hashlib,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).parent))
from native_round_trip import inputs_digest

def main():
    p=argparse.ArgumentParser(description=__doc__.splitlines()[0]);p.add_argument('--ibcmd-rs',required=True,type=pathlib.Path);p.add_argument('--root',required=True,type=pathlib.Path);p.add_argument('--out',required=True,type=pathlib.Path);p.add_argument('--set',action='append',default=[],help='Exact platform/set path to measure; repeat to select more');a=p.parse_args()
    a.out.mkdir(parents=True,exist_ok=False)
    result={'binary_sha256':hashlib.sha256(a.ibcmd_rs.read_bytes()).hexdigest(),'sql_writes':False,'import_scope':'XML compilation only, not SQL import/apply','sets':[]}
    for mp in sorted(a.root.glob('*/corpus-*/manifest.json')):
        m=json.loads(mp.read_text(encoding='utf-8'));s=mp.parent
        if a.set and s.relative_to(a.root).as_posix() not in a.set:
            continue
        if inputs_digest(s) != m['inputs_sha256']:
            raise SystemExit(f'{s}: inputs changed after native proof; rebuild before measurement')
        dest=a.out/s.parent.name/s.name;dest.mkdir(parents=True)
        cf=dest/'case.cf';raw=base64.b64decode((s/'configuration.cf.b64').read_bytes())
        if hashlib.sha256(raw).hexdigest() != m['configuration_cf']['decoded_sha256']:
            raise SystemExit(f'{s}: CF digest differs from the native proof manifest')
        cf.write_bytes(raw)
        out=dest/'export'
        def run(step,args):
            full=[str(a.ibcmd_rs),*map(str,args)];r=subprocess.run(full,capture_output=True)
            (dest/(step+'.stdout.log')).write_bytes(r.stdout);(dest/(step+'.stderr.log')).write_bytes(r.stderr)
            return {'exit_code':r.returncode,'arguments':list(map(str,args)),'stderr':r.stderr.decode('utf-8',errors='replace')}
        export=run('cf-export',['cf','export',cf,out,'--source-version',m['evidence']['source_version']])
        expected={p.relative_to(s/'native').as_posix():p.read_bytes() for p in (s/'native').rglob('*') if p.is_file() and p.name!='ConfigDumpInfo.xml'}
        actual={p.relative_to(out).as_posix():p.read_bytes() for p in out.rglob('*') if p.is_file() and p.name!='ConfigDumpInfo.xml'}
        changes=[{'path':k,'status':'missing' if k not in actual else 'extra' if k not in expected else 'different'} for k in sorted(expected.keys()|actual.keys()) if expected.get(k)!=actual.get(k)]
        empty=dest/'empty-rows';empty.mkdir()
        audit=run('xml-compile',['audit-empty-stage',s/'native',empty,'--platform',m['evidence']['platform_version'],'--max-samples',str((1 << 64)-1),'--output',dest/'audit.json'])
        report=json.loads((dest/'audit.json').read_text(encoding='utf-8')) if (dest/'audit.json').exists() else None
        failures=[v for k,values in (report or {}).get('samples',{}).items() if k.startswith('failed |') for v in values]
        item={'set':str(s.relative_to(a.root)).replace('\\','/'),'native_proven':sum(v['status']=='platform_proven' for v in m['variants']),'platform_refused':[v['name'] for v in m['variants'] if v['status']=='platform_refused'],'export':export,'export_differences':changes,'xml_compile':audit,'xml_compile_failure_count':(report or {}).get('failures'),'xml_compile_failures':failures}
        item.update(manifest_sha256=hashlib.sha256(mp.read_bytes()).hexdigest(), inputs_sha256=m['inputs_sha256'], native_cf_sha256=m['configuration_cf']['decoded_sha256'])
        result['sets'].append(item)
        print(item['set'],f"export={export['exit_code']} differences={len(changes)} compile_failures={item['xml_compile_failure_count']}",flush=True)
    if not result['sets']:
        raise SystemExit('no selected, platform-proven set was found')
    (a.out/'measurements.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
if __name__=='__main__':main()
