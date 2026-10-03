"""Bounded read-only physical metadata and prior613 byte reconciliation."""
import concurrent.futures,hashlib,json,os,pathlib,time
root=pathlib.Path('F:/ibcmd/lab/05/wave3/load/evidence/compact6-case5-prior')
lab=root.parents[1];deadline=time.monotonic()+240;cap=65536
def attrs(p):
    s=p.lstat()
    if getattr(s,'st_file_attributes',0)&1024:raise ValueError('reparse '+str(p))
    return s
for p in (root,*root.parents):attrs(p)
proofpath=lab/'compact6-runtime-resource-proof.json'
rawproof=proofpath.read_bytes()
assert hashlib.sha256(rawproof).hexdigest().upper()=='72D05BACA921E350773C05CDD5F1616A18FE0961282FB81CC3950EAAB0B1E450'
proof=json.loads(rawproof)
entries=proof['raw'];assert len(entries)==613
files={};dirs=[];stack=[root];count=0;empty=0
while stack:
    p=stack.pop();assert time.monotonic()<deadline
    s=attrs(p);count+=1;assert count<=cap
    children=list(os.scandir(p));empty+=not children
    dirs.append({'path':str(p),'attributes':getattr(s,'st_file_attributes',0),'mtime_ns':s.st_mtime_ns,'members':len(children)})
    for e in children:
        q=pathlib.Path(e.path);meta=attrs(q)
        if e.is_dir(follow_symlinks=False):stack.append(q)
        elif e.is_file(follow_symlinks=False):
            count+=1;assert count<=cap
            key=str(q).casefold();assert key not in files
            files[key]={'path':str(q),'bytes':meta.st_size,'attributes':getattr(meta,'st_file_attributes',0)}
        else:raise ValueError('unknown member')
assert len(files)==597 and len(dirs)==4888
expected={str(pathlib.Path(e['file'])).casefold() for e in entries if pathlib.Path(e['file']).is_relative_to(root)}
assert len(expected)==597 and expected==set(files)
def verify(e):
    assert time.monotonic()<deadline
    p=pathlib.Path(e['file'])
    assert p.is_absolute()
    for q in (p,*p.parents):attrs(q)
    before=attrs(p);assert before.st_size==e['bytes'] and before.st_size<=1024**3
    h=hashlib.sha256();n=0
    with p.open('rb') as f:
        while True:
            b=f.read(1024*1024)
            if not b:break
            n+=len(b);assert n<=before.st_size and time.monotonic()<deadline;h.update(b)
    after=attrs(p)
    assert (before.st_size,before.st_mtime_ns)==(after.st_size,after.st_mtime_ns)
    assert n==e['bytes'] and h.hexdigest().upper()==e['sha256']
    return {'file':str(p),'bytes':n,'sha256':h.hexdigest().upper()}
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:verified=list(pool.map(verify,entries))
out={'scope':'READONLY census/reconciliation; no script guard change/retry/queue/SQL/native/process/move/delete',
     'entry_cap':cap,'ordinary_files':len(files),'directories_including_root':len(dirs),'empty_directories':empty,
     'total_nodes':count,'original_limit':2048,'no_reparse':True,'exact597_file_set':True,'all613_bytes_equal':True,
     'file_bytes':sum(e['bytes'] for e in files.values()),'directories':sorted(dirs,key=lambda x:x['path'].casefold()),
     'verified_prior613':verified,'prior613_manifest_sha256':hashlib.sha256(rawproof).hexdigest().upper()}
dest=lab/'compact8-prior-archive-census-v1.json'
with dest.open('xb') as f:f.write(json.dumps(out,indent=2).encode()+b'\n')
print('PASS exact5485 nodes (4888 dirs inclroot/597 files), all613 bytes+paths unchanged, emptydirs',empty,'noReparse; read-only')
