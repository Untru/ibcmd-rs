import argparse,hashlib,importlib.util,json,pathlib,sys
sys.dont_write_bytecode=True
root=pathlib.Path('F:/ibcmd/lab/05/wave3/load')
spec=importlib.util.spec_from_file_location('r',pathlib.Path(__file__).parent/'read_compact8_recovery.py');r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
a=argparse.ArgumentParser();a.add_argument('--live',type=pathlib.Path,required=True);a.add_argument('--manifest',type=pathlib.Path,required=True);a.add_argument('--storage',type=pathlib.Path,required=True);a.add_argument('--output',type=pathlib.Path,required=True);x=a.parse_args()
for p in (x.live,x.manifest,x.storage,x.output):
 if not p.is_absolute() or not p.resolve().is_relative_to(root.resolve()):raise ValueError('fixed owned F root required')
r.verify(x.manifest,x.live,x.storage)
m=r.load(x.live,2*1024*1024);p=m['payload'];side=x.live.parent/p['recovery_file'];q=r.load(side,2*1024*1024)['payload'];pack=side.parent/q['pack_file'];originals=[x.live,x.manifest,side,pack,x.storage];before={str(f):r.sha(f.read_bytes()) for f in originals}
parent=root/'compact8-invalid-actual';parent.mkdir(exist_ok=False);cases=[]
for name in ('corrupt-pack','missing-pack','wrong-envelope-digest','wrong-sidecar-digest','foreign-sidecar-name'):
 d=parent/name;d.mkdir();copy=json.loads(json.dumps(m));(d/side.name).write_bytes(side.read_bytes());(d/pack.name).write_bytes(pack.read_bytes())
 if name=='corrupt-pack':
  b=bytearray(pack.read_bytes());b[0]^=1;(d/pack.name).write_bytes(b)
 elif name=='missing-pack':(d/pack.name).unlink()
 elif name=='wrong-envelope-digest':copy['integrity_sha256']='0'*64
 elif name=='wrong-sidecar-digest':copy['payload']['recovery_file_sha256']='0'*64;copy['integrity_sha256']=r.sha(r.encoded(copy['payload']))
 else:copy['payload']['recovery_file']='foreign.recovery.json';copy['integrity_sha256']=r.sha(r.encoded(copy['payload']))
 artifact=d/'invalid.live.json';artifact.write_bytes(r.encoded(copy));cases.append({'name':name,'artifact':str(artifact),'sha256':r.sha(artifact.read_bytes()),'files_sha256':{f.name:r.sha(f.read_bytes()) for f in d.iterdir()}})
assert before=={str(f):r.sha(f.read_bytes()) for f in originals}
with x.output.open('x',encoding='utf-8') as f:json.dump({'cases':cases,'originals_sha256':before,'runtime_originals_preserved':True},f,indent=2)
print('PASS actual immutable package copies; original SHA unchanged, no native or SQL')
