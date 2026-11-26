import argparse, hashlib, json, pathlib, re, sys
sys.dont_write_bytecode=True

def sha(b): return hashlib.sha256(b).hexdigest()
def encoded(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':')).encode()
def require(ok, why):
    if not ok: raise ValueError(why)
def bounded(path, cap):
    require(path.is_absolute() and path.is_file() and not path.is_symlink() and path.stat().st_size <= cap, 'file/budget')
    for probe in (path,*path.parents):
        if probe.exists(): require(not (getattr(probe.lstat(),'st_file_attributes',0) & 1024),'reparse input ancestry')
    with path.open('rb') as f: b=f.read(cap+1)
    require(len(b)<=cap, 'file grew'); return b


def unique(pairs):
    out={}
    for k,v in pairs:
        require(k not in out,'duplicate JSON field '+k);out[k]=v
    return out

def load(path,cap): return json.loads(bounded(path,cap),object_pairs_hook=unique)
def exact(value,keys,label): require(isinstance(value,dict) and set(value)==set(keys),'unknown/missing '+label+' fields')
def read_live(path):
    m=load(path,2*1024*1024);exact(m,('format','integrity_sha256','payload'),'LIVE manifest');require(m['format']==2,'compact LIVE format required');p=m['payload']
    keys=('sql_engine_version','verified_platform_profile','storage_schema_sha256','operation','identity','tail','recovery_token','cluster_id','infobase_id','recovery_file','recovery_file_sha256');exact(p,keys,'envelope')
    exact(p['identity'],('server','database','database_guid','family_guid','recovery_fork'),'identity')
    require(sha(encoded(p))==m['integrity_sha256'],'LIVE envelope integrity')
    require(p['sql_engine_version']=='17.0.1135.8' and p['verified_platform_profile']=='platform-8.3.27.2214','supported measured profile')
    require(p['identity']['database']=='ibcmd_rs_05_load_w3_compact9_20261002','owned database')
    require(re.fullmatch('[a-fA-F0-9]{64}',p['storage_schema_sha256']) is not None,'storage schema digest')
    tail=pathlib.Path(p['tail']);require(tail.is_absolute() and tail.resolve().is_relative_to(pathlib.Path('F:/ibcmd/lab/05/wave3/load').resolve()),'owned tail path')
    for key in ('operation','cluster_id','infobase_id'):
        require(re.fullmatch(r'[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}',p[key]) is not None,'UUID '+key)
    for key in ('database_guid','family_guid','recovery_fork'):
        require(re.fullmatch(r'[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}',p['identity'][key]) is not None,'identity UUID '+key)
    require(re.fullmatch('[a-fA-F0-9]{64}',p['recovery_token']) is not None,'token hex')
    require(p['recovery_file']=='ibcmd-live-'+p['recovery_token'].lower()+'.recovery.json','exact token sidecar basename')
    side=path.parent/p['recovery_file'];require(sha(bounded(side,2*1024*1024))==p['recovery_file_sha256'],'sidecar file digest')
    side_manifest=load(side,2*1024*1024);exact(side_manifest,('format','integrity_sha256','payload'),'sidecar manifest');q=side_manifest['payload']
    exact(q,('database','mode','recovery_token','old_generation','new_generation','pack_file','pack_bytes','pack_sha256','overwritten_config_rows','retained_config_rows','prior_config_dynamically_updated','prior_params_dynamically_updated','staged_rows'),'sidecar payload')
    require(side_manifest['format']==2 and sha(encoded(q))==side_manifest['integrity_sha256'],'sidecar integrity')
    require(q['database']==p['identity']['database'] and q['mode']=='live' and q['recovery_token'].lower()==p['recovery_token'].lower(),'sidecar DB/mode/token')
    require(re.fullmatch('[a-f0-9]{64}',q['pack_sha256']) is not None and q['pack_file']=='ibcmd-recovery-'+q['pack_sha256']+'.pack','digest pack basename')
    pack=bounded(side.parent/q['pack_file'],96*1024*1024);require(len(pack)==q['pack_bytes'] and sha(pack)==q['pack_sha256'],'LIVE whole pack')
    recovered={'old_generation':q['old_generation'],'new_generation':q['new_generation']};cursor=0;seen=set()
    for key,single in [('overwritten_config_rows',False),('retained_config_rows',False),('prior_config_dynamically_updated',True),('prior_params_dynamically_updated',True),('staged_rows',False)]:
        items=([q[key]] if q[key] is not None else []) if single else q[key];out=[]
        require(isinstance(items,list) and len(items)<=4096,'row inventory budget')
        for r in items:
            exact(r,('file_name','part_no','creation','modified','attributes','data_size','offset','length','sha256'),'packed row')
            require(type(r['offset']) is int and type(r['length']) is int and r['offset']==cursor and 0<=r['length']<=16*1024*1024 and r['data_size']==r['length'],'LIVE contiguous range')
            pair=(key,r['file_name'],r['part_no']);require(pair not in seen,'duplicate packed key');seen.add(pair)
            b=pack[cursor:cursor+r['length']];require(len(b)==r['length'] and sha(b)==r['sha256'],'LIVE row digest');cursor+=r['length']
            out.append({k:r[k] for k in ('file_name','part_no','creation','modified','attributes','data_size')}|{'binary_data':list(b)})
        if key!='retained_config_rows' or out: recovered[key]=out[0] if single and out else (None if single else out)
    require(cursor==len(pack),'LIVE unreferenced pack bytes')
    require(sha(encoded(recovered)).lower()==p['recovery_token'].lower(),'reconstructed token')
    return {'format':1,**{k:p[k] for k in keys if k not in ('recovery_file','recovery_file_sha256')},'recovery':recovered}

def verify(manifest, live, storage):
    m=load(manifest, 2*1024*1024);exact(m,('format','integrity_sha256','payload'),'standalone manifest');p=m['payload']
    exact(p,('database','mode','recovery_token','old_generation','new_generation','pack_file','pack_bytes','pack_sha256','overwritten_config_rows','retained_config_rows','prior_config_dynamically_updated','prior_params_dynamically_updated','staged_rows'),'standalone payload')
    require(m['format']==2 and sha(encoded(p))==m['integrity_sha256'], 'manifest integrity')
    require(re.fullmatch(r'[a-f0-9]{64}', p['pack_sha256']) is not None, 'pack SHA')
    require(p['pack_file']=='ibcmd-recovery-'+p['pack_sha256']+'.pack', 'adjacent digest pack name')
    packpath=manifest.parent/p['pack_file']; pack=bounded(packpath, 96*1024*1024)
    require(len(pack)==p['pack_bytes'] and sha(pack)==p['pack_sha256'], 'whole pack hash/size')
    l=read_live(live); require(l['format']==1, 'reconstructed legacy LIVE format')
    require(p['database']==l['identity']['database'] and p['mode']=='live', 'database/mode')
    require(p['recovery_token'].lower()==l['recovery_token'].lower(), 'token identity')
    s={}
    for line in bounded(storage,1216*1024*1024).decode('utf-8-sig').splitlines():
        fields=line.split('|')
        if len(fields)==9 and fields[0] in ('Config','ConfigSave','Params'):
            require(tuple(fields[:3]) not in s, 'duplicate storage key'); s[tuple(fields[:3])]=fields[3:]
    cursor=0; verified=0; recovered={}
    for key, table, singleton in [('overwritten_config_rows','Config',False),('retained_config_rows','Config',False),('prior_config_dynamically_updated','Config',True),('prior_params_dynamically_updated','Params',True),('staged_rows','ConfigSave',False)]:
        rows=([p[key]] if p[key] is not None else []) if singleton else p[key]
        out=[]
        for r in rows:
            exact(r,('file_name','part_no','creation','modified','attributes','data_size','offset','length','sha256'),'standalone packed row')
            require(r['offset']==cursor and 0<=r['length']<=16*1024*1024 and r['data_size']==r['length'], 'range/size')
            b=pack[cursor:cursor+r['length']]; require(len(b)==r['length'] and sha(b)==r['sha256'], 'row digest'); cursor+=r['length']
            physical=s[(table,r['file_name'],str(r['part_no']))]
            # SQL datetime rendering and Rust representation both use style 121.
            require(physical==[r['creation'],r['modified'],str(r['attributes']),str(r['data_size']),str(len(b)),sha(b).upper()], 'full physical preimage mismatch')
            out.append({k:r[k] for k in ('file_name','part_no','creation','modified','attributes','data_size')}|{'binary_data':list(b)})
            verified+=1
        recovered[key]=(out[0] if out else None) if singleton else out
        require(recovered[key]==l['recovery'].get(key, [] if not singleton else None), 'standalone/LIVE sidecar row mismatch')
    require(cursor==len(pack), 'unreferenced pack suffix')
    require(p['old_generation']==l['recovery']['old_generation'] and p['new_generation']==l['recovery']['new_generation'], 'generation mismatch')
    require(sha(encoded(l['recovery'])).lower()==l['recovery_token'].lower(), 'legacy snapshot token')
    return {'sidecar_file':load(live,2*1024*1024)['payload']['recovery_file'],'envelope_identity':l['identity'],'database':p['database'],'compact_format':2,'compact_live_format':2,'reconstructed_live_format':1,'pack_sha256':sha(pack),'manifest_sha256':sha(bounded(manifest,2*1024*1024)),'live_sha256':sha(bounded(live,64*1024*1024)),'storage_sha256':sha(bounded(storage,1216*1024*1024)),'verified_full_header_rows':verified,'token':l['recovery_token'],'no_sql_or_undo':True}

if __name__=='__main__':
    a=argparse.ArgumentParser(); a.add_argument('--manifest',type=pathlib.Path,required=True); a.add_argument('--live',type=pathlib.Path,required=True); a.add_argument('--storage',type=pathlib.Path,required=True); a.add_argument('--output',type=pathlib.Path,required=True); x=a.parse_args()
    report=verify(x.manifest,x.live,x.storage)
    with x.output.open('xb') as f: f.write(json.dumps(report,ensure_ascii=False,indent=2).encode()+b'\n')
    print('PASS compact LIVE2 envelope/sidecar/pack/fullheaders and standalone2 reconstructed token; read-only inputs')


