import datetime,json,pathlib,statistics,re,sys
lab=pathlib.Path(sys.argv[1])
clients={}
for p in sorted((lab/'obs').glob('*.log')):
    events=[];ops=[];errors=[];views=set()
    for line in p.read_text(encoding='utf-8-sig').splitlines():
        fields=line.split('|',6)
        if len(fields)!=7:raise ValueError(f'bad journal line {p}')
        stamp,label,event,sid,client,server,detail=fields
        values=dict(re.findall(r'(\w+)=([^;]+)',detail))
        if client or server:views.add((client,server))
        if event=='operation':
            ops.append({'stamp':int(stamp),'committed':values.get('committed'),'report_ok':values.get('report_ok'),'uuid':values.get('doc_uuid'),'client_ms':int(values.get('client_ms','0'))})
        if 'error' in event or 'error=' in detail or values.get('committed')=='unknown' or (event=='operation' and values.get('report_ok')!='1'):errors.append(line)
        events.append(event)
    committed=[o for o in ops if o['committed']=='1']
    lat=[o['client_ms'] for o in committed]
    clients[p.stem]={'events':len(events),'operations':len(ops),'committed':len(committed),'report_ok':sum(o['report_ok']=='1' for o in committed),'unknown_commit':sum(o['committed']=='unknown' for o in ops),'errors':errors,'views':sorted(views),'doc_uuids':[o['uuid'] for o in committed], 'latency_ms':{'min':min(lat),'median':statistics.median(lat),'p95':sorted(lat)[min(len(lat)-1,int(len(lat)*.95))],'max':max(lat)} if lat else None}
loaded=['base1','base2','base3']
all_ids=[u for l in loaded for u in clients[l]['doc_uuids']]
if len(set(all_ids))!=len(all_ids):raise ValueError('duplicate committed document UUID')
output={'scope':'3 real loaded clients; five own dynamic publications and two native controls, bounded BSP invoice/report cohort; not throughput ceiling or LIVE acceptance','clients':clients,'final_harness_loaded':loaded,'committed_total':sum(clients[l]['committed'] for l in loaded),'report_ok_total':sum(clients[l]['report_ok'] for l in loaded),'all_errors_preserved':True,'five_own_cases':[]}
for case in range(1,6):
    tag=f'own{case}';r=json.loads((lab/'logs'/f'{tag}-apply.result.json').read_text(encoding='utf-8-sig'))
    n=json.loads((lab/'logs'/f'{tag}-repeat.result.json').read_text(encoding='utf-8-sig'))
    output['five_own_cases'].append({'tag':tag,'exit':r['exit_code'],'elapsed_ms':r['elapsed_ms'],'repeat_exit':n['exit_code'],'repeat_storage_exact':(lab/'snapshots'/f'{tag}-after-storage.txt').read_bytes()==(lab/'snapshots'/f'{tag}-repeat-after-storage.txt').read_bytes()})
(lab/'workload-summary.json').write_text(json.dumps(output,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'committed':output['committed_total'],'reports':output['report_ok_total'],'errors':{l:len(clients[l]['errors']) for l in loaded},'latency_ms':{l:clients[l]['latency_ms'] for l in loaded},'views':{l:clients[l]['views'] for l in loaded},'own':output['five_own_cases']},ensure_ascii=False,indent=2))
# Independently reconcile the actual posted document UUIDs read via a fresh COM session.
actual={}
for line in (lab/'snapshots'/'committed-documents-v5.txt').read_text(encoding='utf-8-sig').splitlines():
    if not line:continue
    mark,uuid,posted=line.split('|')
    if posted!='True':raise ValueError('unposted workload document')
    if uuid in actual:raise ValueError('duplicate persisted UUID')
    actual[uuid]=mark
if set(actual)!=set(all_ids):raise ValueError(f'persisted UUID mismatch: actual={len(actual)} journal={len(all_ids)}')
output['independently_reconciled_posted_documents']=len(actual)
output['persisted_uuid_set_equals_journal']=True
(lab/'workload-summary.json').write_text(json.dumps(output,ensure_ascii=False,indent=2),encoding='utf-8')
print('PASS persisted exact UUID set and posted flags:',len(actual))
# Attribute load samples to actual command intervals / the following generation window.
def stamp_utc(value):
    d=datetime.datetime.fromisoformat(value.replace('Z','+00:00')).astimezone(datetime.timezone.utc)
    return int((d-datetime.datetime(1,1,1,tzinfo=datetime.timezone.utc)).total_seconds()*1000)
bounds=[]
for tag in ['native1']+[f'own{i}' for i in range(1,6)]+['native2']:
    if tag.startswith('native'):
        before=json.loads((lab/'logs'/f'{tag}-before-command.json').read_text(encoding='utf-8-sig'))['utc']
        after=json.loads((lab/'logs'/f'{tag}-after-command.json').read_text(encoding='utf-8-sig'))['utc']
    else:
        before=json.loads((lab/'logs'/f'{tag}-apply.command.json').read_text(encoding='utf-8-sig'))['started_utc']
        after=json.loads((lab/'logs'/f'{tag}-apply.result.json').read_text(encoding='utf-8-sig'))['ended_utc']
    bounds.append((tag,stamp_utc(before),stamp_utc(after)))
bounds.sort(key=lambda b:b[1]);phases={}
for label in loaded:
    for line in (lab/'obs'/f'{label}.log').read_text(encoding='utf-8-sig').splitlines():
        f=line.split('|',6)
        if f[2]!='operation':continue
        values=dict(re.findall(r'(\w+)=([^;]+)',f[6]));t=int(f[0]);phase='baseline'
        for tag,start,end in bounds:
            if t>=start:phase=tag+('-during' if t<=end else '-after')
        p=phases.setdefault(phase,{'attempts':0,'committed':0,'reports':0,'latencies':[]});p['attempts']+=1
        if values.get('committed')=='1':p['committed']+=1;p['latencies'].append(int(values['client_ms']))
        if values.get('report_ok')=='1':p['reports']+=1
for p in phases.values():
    lat=p.pop('latencies');p['client_latency_ms']={'median':statistics.median(lat),'p95':sorted(lat)[min(len(lat)-1,int(len(lat)*.95))],'max':max(lat)} if lat else None
output['phases']=phases
output['phase_notes']='During uses journal completed-call timestamps within command wall bookends (includes FIFO acquisition for native); after lasts until the next activation begins. Native2 is an order/repetition control on the same clone after own5, not a byte-identical restored twin.'
(lab/'workload-summary.json').write_text(json.dumps(output,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(phases,ensure_ascii=False,indent=2))
