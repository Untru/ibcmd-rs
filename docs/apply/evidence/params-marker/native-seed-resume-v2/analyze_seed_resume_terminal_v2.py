"""Offline actual native six-operation seed; verify retained raw packs, no DB calls."""
import sys
sys.dont_write_bytecode=True
import collections,hashlib,json,pathlib,re
from check_seed_stage import Image,inflate,ID
from compare_seed_resume_v2 import EXPECTED
from classify_seed_shape_resume_v2 import classify

LAB=pathlib.Path('F:/ibcmd/lab/05/wave3/params-marker')
RUN=LAB/'native-seed-resume-v2'
OUT=LAB/'native-seed-resume-V2-terminal-analysis-v1.json'
DB='ibcmd_rs_05_marker_w3_seed_c9e54c'
if OUT.exists():raise ValueError('fresh offline result required')
def digest(value):return hashlib.sha256(json.dumps(value,sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()
def no_reparse(p):
    for a in (p,*p.parents):
        if a.exists() and a.lstat().st_file_attributes&1024:raise ValueError('reparse evidence')
def read(p,limit=64*1024*1024):
    no_reparse(p)
    if p.stat().st_size>limit:raise ValueError('bounded artifact required')
    return json.loads(p.read_text(encoding='utf-8-sig'))
terminal=read(LAB/'native-seed-resume-v2-launch/terminal.json')
final=read(RUN/'final-state.json');outcome=read(RUN/'outcome.json')
if terminal['pid']!=49788 or terminal['exit']!=0 or terminal['execution_state']!='known_terminal' or terminal['manifest_sha256']!='44B76309AB13DA33EB0A3B97BAEF3D4AD80335B695387B4EA7D119A49EC5532C':raise ValueError('original terminal differs')
if final['database']!=DB or final['native_potential'] or final['uncertain'] or final['bound_names'] or final['worker_used'] or not final['no_restore_in_resume']:raise ValueError('terminal resource/identity differs')
receipts=sorted(p for p in RUN.glob('*.json') if re.fullmatch(r'\d{3}-.+',p.stem) and not p.name.endswith('.command.json'))
if len(receipts)!=117 or [int(p.name[:3]) for p in receipts]!=list(range(1,118)):raise ValueError('exact117 known steps required')
commands=[]
for p in receipts:
    value=read(p,1024*1024)
    if type(value['ExitCode']) is not int or value['ExitCode']!=0:raise ValueError('nonzero/unknown step')
    command=read(p.with_name(p.stem+'.command.json'),1024*1024)
    if re.fullmatch(r'\d{3}-(?:force_b|force_c|fold_d)-(?:import|apply)',p.stem):
        args=command['arguments']
        if command['executable']!='C:\\Program Files\\1cv8\\8.3.27.2214\\bin\\ibcmd.exe' or '--db-name='+DB not in args:raise ValueError('native source/target differs')
        commands.append({'leaf':p.name,'command':command,'known_exit':0})
if len(commands)!=6 or len([p for p in receipts if re.fullmatch(r'\d{3}-(?:force_b|force_c|fold_d)-(?:import|apply)-release',p.stem)])!=6:raise ValueError('exact six writers/releases required')
tags=['seed-resume-initial-v2']
for phase in ('force_b','force_c','fold_d'):
    tags += [phase+'-import-prequeue-v2',phase+'-import-postqueue-v2',phase+'-staged-v2',phase+'-apply-prequeue-v2',phase+'-apply-postqueue-v2',phase+'-applied-v2']
tags+=['seed-resume-after-backup-v2']
fingerprints={};verified=[];phase_semantics=[]
for tag in ['seed-cold-settle-after-v1',*tags]:
    image=Image(tag);count=total=groups=0
    if set(image.data['storage'])!={'Config','ConfigSave','Params','Files','ConfigCAS','ConfigCASSave'} or len(image.data['auxiliary'])!=10:raise ValueError('full6+10 required')
    for table,info in image.data['storage'].items():
        pack=image.folder/info['pack'];no_reparse(pack)
        if info['pack']!=table+'.pack' or pack.stat().st_size!=info['bytes']:raise ValueError('pack identity/length')
        h=hashlib.sha256();offset=0;parts=collections.defaultdict(list)
        with pack.open('rb') as stream:
            for row in info['rows']:
                if row['offset']!=offset or not 0<=row['byte_len']<=64*1024*1024:raise ValueError('row range/budget')
                data=stream.read(row['byte_len'])
                if len(data)!=row['byte_len'] or hashlib.sha256(data).hexdigest()!=row['sha256']:raise ValueError('raw row SHA')
                h.update(data);offset+=len(data);count+=1;parts[row['name']].append(row)
            if stream.read(1):raise ValueError('unclassified trailing pack bytes')
        if offset!=info['bytes'] or h.hexdigest()!=info['pack_sha256']:raise ValueError('whole pack SHA')
        for rows in parts.values():
            rows.sort(key=lambda r:r['part'])
            if [r['part'] for r in rows]!=list(range(len(rows))) or len({r['data_size'] for r in rows})!=1 or rows[0]['data_size']!=sum(r['byte_len'] for r in rows):raise ValueError('multipart full size/continuity')
            groups+=1
        total+=offset
    aux={}
    for name,value in image.data['auxiliary'].items():
        rows=collections.Counter(json.dumps(r,sort_keys=True,ensure_ascii=False,separators=(',',':')) for r in value['rows'])
        aux[name]=digest({'columns':value['columns'],'rows':sorted(rows.items())})
    fingerprints[tag]=(digest(image.data['storage']),aux)
    verified.append({'tag':tag,'rows':count,'bytes':total,'groups':groups,'full_headers_and_raw_ranges_and_pack_SHA_verified':True,'auxiliary_tables':10})
    if tag in ('force_b-staged-v2','force_c-staged-v2','fold_d-staged-v2'):
        prior={'force_b-staged-v2':'seed-resume-initial-v2','force_c-staged-v2':'force_b-applied-v2','fold_d-staged-v2':'force_c-applied-v2'}[tag]
        before=Image(prior);staged_names=[r['name'] for r in image.data['storage']['ConfigSave']['rows']]
        descriptor_present=ID in staged_names
        phase_semantics.append({'tag':tag,'stage_names':staged_names,'descriptor_present':descriptor_present,'descriptor_decoded_equal':inflate(before.effective(ID))==inflate(image.blob('ConfigSave',ID)) if descriptor_present else None,'body_raw_changed':before.effective(ID+'.0')!=image.blob('ConfigSave',ID+'.0')})
def same(a,b):
    if fingerprints[a]!=fingerprints[b]:raise ValueError('full6+10 differs: '+a+' -> '+b)
same('seed-cold-settle-after-v1','seed-resume-initial-v2')
for label,expected in EXPECTED.items():same(expected,label+'-prequeue-v2');same(label+'-prequeue-v2',label+'-postqueue-v2')
same('fold_d-applied-v2','seed-resume-after-backup-v2')
last=Image('fold_d-applied-v2');marker=last.blob('Params','DynamicallyUpdated');shape=classify(last.data,marker)
if not shape['usable'] or len(shape['history'])!=4 or any(r['name']=='deleted' for r in last.data['storage']['Config']['rows']):raise ValueError('settled seed observation differs')
header=read(RUN/'seed-header.json',1024*1024);backup=LAB/'baselines/marker-seed-resumed-v2.bak';no_reparse(backup)
h=hashlib.sha256()
with backup.open('rb') as stream:
    for chunk in iter(lambda:stream.read(1024*1024),b''):h.update(chunk)
if header['sha256']!=h.hexdigest() or outcome['backup_sha256'].lower()!=h.hexdigest() or header['bytes']!=backup.stat().st_size or len(header['rows'])!=1:raise ValueError('preserved backup SHA/size')
r=header['rows'][0]
if r['DatabaseName']!=DB or r['BackupType']!=1 or r['Position']!=1 or r['IsDamaged'] or not r['IsCopyOnly'] or not r['HasBackupChecksums']:raise ValueError('native full COPY_ONLY checksum header')
result={'scope':'ONE actual native Bforce/Cforce/Ddisable natural seed, not N1/OWN product marker admission','original_terminal':terminal,'final_state':final,'known_steps':117,'native_commands':commands,'known_native_releases':6,'verified_snapshots':verified,'exact_comparisons':{'initial':True,'last_judged_to_prequeue':6,'prequeue_to_postqueue':6,'backup_before_after':True,'normalizations':[]},'staged_semantics':phase_semantics,'final_shape':shape,'marker_utf8':marker.decode('utf-8-sig'),'backup_header':header,'product_rule_enabled':False,'N1_matrix_unexecuted':True,'OWN_unexecuted':True}
with OUT.open('x',encoding='utf-8') as stream:json.dump(result,stream,ensure_ascii=False,indent=2);stream.write('\n')
print(json.dumps({'known_steps':117,'native_writers':6,'native_releases':6,'verified_snapshots':len(verified),'marker_bytes':len(marker),'history':shape['history'],'staged_semantics':phase_semantics,'backup_sha256':h.hexdigest(),'normalizations':[]},ensure_ascii=False))
