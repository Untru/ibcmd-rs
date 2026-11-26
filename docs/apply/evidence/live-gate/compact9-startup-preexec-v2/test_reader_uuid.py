"""Actual retained Case5 inputs, virtual copied DB9 package; no filesystem fixtures."""
import copy, importlib.util, json, pathlib, sys
sys.dont_write_bytecode = True
root = pathlib.Path('F:/ibcmd/lab/05/wave3/load')
spec = importlib.util.spec_from_file_location('r', pathlib.Path(__file__).parent/'read_compact9_recovery.py')
r = importlib.util.module_from_spec(spec); spec.loader.exec_module(r)
live = root/'compact5-load-5-r1.recovery.live.json'
standalone = root/'compact5-load-5-r1.recovery.json'
storage = root/'snapshots/compact5-load-5-staged-storage.txt'
original = r.load(live, 2*1024*1024)
side = root/original['payload']['recovery_file']; q = r.load(side, 2*1024*1024)
pack = root/q['payload']['pack_file']
inputs = (live, standalone, storage, side, pack)
raw = {p:r.bounded(p,1216*1024*1024) for p in inputs}
before = {str(p):r.sha(b) for p,b in raw.items()}
header = r.load(root/'logs/compact5-load-5-header-1.result.json', 2*1024*1024)
assert header['ExitCode']==0
sets = [line.split('|') for line in header['Stdout'].splitlines() if line.startswith('ibcmd-rs:live:')]
assert len(sets)==1
fields = ['operation','cluster_id','infobase_id','database_guid','family_guid','recovery_fork']
cases = ['upper','lower','mixed','foreign-db5','foreign-db7',*['malformed-'+k for k in fields],'foreign-valid-guid','duplicate-standalone','unknown-standalone']
d = root/'virtual-compact9-no-files-created'
for case in cases:
    copied_side = copy.deepcopy(q); modified=copy.deepcopy(original)
    db = ('ibcmd_rs_05_load_w3_compact5_20261001' if case=='foreign-db5' else
          'ibcmd_rs_05_load_w3_compact7_20261002' if case=='foreign-db7' else
          'ibcmd_rs_05_load_w3_compact9_20261002')
    copied_side['payload']['database']=db
    copied_side['integrity_sha256']=r.sha(r.encoded(copied_side['payload']))
    side_bytes=r.encoded(copied_side)
    p=modified['payload'];p['identity']['database']=db;p['tail']=str(root/'synthetic-compact9-only.trn')
    p['recovery_file_sha256']=r.sha(side_bytes)
    for key in fields:
        target=p if key in fields[:3] else p['identity'];value=target[key]
        target[key]=(value.upper() if case=='upper' else value.lower() if case=='lower' else
                     ''.join(c.upper() if i%2 else c.lower() for i,c in enumerate(value)))
    if case.startswith('malformed-'):
        key=case.removeprefix('malformed-');target=p if key in fields[:3] else p['identity'];target[key]='Z'+target[key][1:]
    if case=='foreign-valid-guid':p['identity']['database_guid']='00000000-0000-0000-0000-000000000000'
    modified['integrity_sha256']=r.sha(r.encoded(p))
    standalone_bytes=side_bytes
    if case=='duplicate-standalone':standalone_bytes=side_bytes.replace(b'{',b'{"format":2,',1)
    if case=='unknown-standalone':standalone_bytes=side_bytes.replace(b'{',b'{"unexpected":0,',1)
    virtual={d/'standalone.json':standalone_bytes,d/original['payload']['recovery_file']:side_bytes,
             d/q['payload']['pack_file']:raw[pack],d/'live.json':r.encoded(modified),storage:raw[storage]}
    def bounded(path,cap):
        b=virtual[path];r.require(len(b)<=cap,'virtual budget');return b
    r.bounded=bounded
    try:
        parsed=r.verify(d/'standalone.json',d/'live.json',storage)
        for key,index in [('database_guid',30),('recovery_fork',31),('family_guid',33)]:
            r.require(parsed['envelope_identity'][key].casefold()==sets[0][index].casefold(),'retained HEADERONLY lineage')
    except ValueError:
        assert case not in ('upper','lower','mixed'),case
    else:
        assert case in ('upper','lower','mixed'),case
        assert parsed['verified_full_header_rows']==10 and parsed['token']==original['payload']['recovery_token']
        assert parsed['envelope_identity']==p['identity']
    print('PASS virtual actual-package DB9/UUID/schema '+case)
assert before=={str(p):r.sha(p.read_bytes()) for p in inputs}
print('PASS originals unchanged; fixtures/cleanup/SQL/native/OS actions0; DB9 runtime UNMEASURED')
