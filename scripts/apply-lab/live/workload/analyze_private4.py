"""Read-only case4 reconciliation; no SQL, native command, or input mutation."""
import argparse
import datetime as dt
import hashlib
import importlib.util
import json
import re
import sys
sys.dont_write_bytecode = True  # Frozen input readers must not create lab cache files.
from pathlib import Path

def read(p): return p.read_text(encoding='utf-8-sig')
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def iso(ms): return (dt.datetime(1, 1, 1, tzinfo=dt.timezone.utc) + dt.timedelta(milliseconds=int(ms))).isoformat()
def fields(detail):
    result = {}
    known = {'attempt', 'client_ms', 'committed', 'doc_uuid', 'doc_ms', 'report_ok', 'report_rows', 'report_ms'}
    for match in re.finditer(r'(?:^|;)([A-Za-z_][A-Za-z0-9_-]*)=([^;]*)', detail):
        key, value = match.groups()
        if key not in known: continue
        if key in result: raise ValueError(f'duplicate receipt field {key}')
        result[key] = value
    if 'committed' in result and result['committed'] not in ('0', '1', 'unknown'):
        raise ValueError('invalid committed value')
    if 'report_ok' in result and result['report_ok'] not in ('0', '1'):
        raise ValueError('invalid report_ok value')
    return result

def reconcile(lab):
    physical = {}
    for line in read(lab/'snapshots/private4-physical-documents.txt').splitlines():
        cells = line.strip().split('|')
        if 'ibcmd-rs-load:' not in line: continue  # sqlcmd's database-context notice
        if len(cells) != 3 or not re.fullmatch('[0-9A-F]{32}', cells[0]):
            raise ValueError('malformed physical workload row')
        raw, posted, comment = cells
        if posted != '01': raise ValueError(f'unposted physical operation {comment}')
        uuid = f'{raw[24:32]}-{raw[20:24]}-{raw[16:20]}-{raw[:4]}-{raw[4:16]}'.lower()
        if comment in physical: raise ValueError(f'duplicate persisted operation {comment}')
        if not re.fullmatch(r'ibcmd-rs-load:private4-load-4-(?:old|new)-[abc]:[1-9][0-9]*', comment): raise ValueError('foreign physical comment')
        if any(v['uuid'] == uuid for v in physical.values()): raise ValueError('duplicate physical UUID')
        physical[comment] = {'uuid':uuid, 'posted':posted}
    
    cohorts = []
    receipts = {}
    unreturned = []
    emitted_errors = []
    for path in sorted((lab/'obs').glob('private4-load-4-*.log')):
        starts, ends, markers = {}, {}, set()
        operations = []
        for line in read(path).splitlines():
            cells = line.split('|', 6)
            if len(cells) != 7: raise ValueError(f'malformed journal {path}:{line}')
            stamp, label, event, sid, client, server, detail = cells
            # Only returned operation receipts have the structured outcome fields.
            # Startup and transport-error descriptions remain free-form evidence.
            values = fields(detail) if event == 'operation' else {}
            if event in ('operation-start', 'call-error'):
                prefix = re.match(r'^attempt=(\d+);', detail)
                if prefix: values['attempt'] = prefix[1]
            if event == 'call-error' and re.match(r'^attempt=\d+;committed=unknown;', detail):
                values['committed'] = 'unknown'
            attempt = values.get('attempt')
            if label != path.stem: raise ValueError(f'journal label differs {path}')
            if event in ('operation-start', 'operation', 'call-error') and (not attempt or not re.fullmatch(r'\d+', attempt)):
                raise ValueError('operation has no attempt')
            if event == 'operation-start':
                if int(attempt) in starts: raise ValueError('duplicate operation start')
                starts[int(attempt)] = line
            if event in ('operation', 'call-error'):
                if int(attempt) not in starts: raise ValueError('operation receipt without start')
                if int(attempt) in ends: raise ValueError('duplicate operation receipt')
                ends[int(attempt)] = line
            if event == 'operation':
                if 'committed' not in values: raise ValueError('operation receipt has no committed value')
                if values['committed'] == '1':
                    uid = values.get('doc_uuid', '')
                    if not re.fullmatch(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', uid): raise ValueError('committed receipt has no UUID')
                    if 'report_ok' not in values: raise ValueError('committed receipt has no report outcome')
                    comment = f'ibcmd-rs-load:{label}:{attempt}'
                    expected = physical.get(comment)
                    if expected != {'uuid':uid, 'posted':'01'}: raise ValueError(f'persisted receipt differs {comment} {expected}')
                    if comment in receipts or uid in receipts.values(): raise ValueError('duplicate committed receipt')
                    receipts[comment] = uid
                    operations.append({'utc':iso(stamp),'attempt':int(attempt),'client':client,'server':server,'uuid':uid, 'report_ok':values['report_ok'] == '1'})
                    markers.add((client,server))
            if 'error' in event or 'error=' in detail or values.get('report_ok') == '0' or values.get('committed') == 'unknown':
                emitted_errors.append(line)
        pending = []
        for number in sorted(set(starts)-set(ends)):
            comment = f'ibcmd-rs-load:{path.stem}:{number}'
            item = {'label':path.stem,'attempt':number,'start':starts[number], 'physical_at_final_readback':physical.get(comment)}
            pending.append(item); unreturned.append(item)
        cohorts.append({'label':path.stem,'starts':len(starts),'confirmed_commits':len(operations),'confirmed_reports':sum(o['report_ok'] for o in operations),'markers':sorted(markers),'unreturned':pending,'operations':operations})
    
    remaining = {c:v for c,v in physical.items() if c not in receipts}
    unreturned_keys={f"ibcmd-rs-load:{o['label']}:{o['attempt']}" for o in unreturned}
    if not set(remaining).issubset(unreturned_keys): raise ValueError('unexplained physical commits')
    if {c['label'] for c in cohorts} != {f'private4-load-4-{age}-{letter}' for age in ('old','new') for letter in 'abc'}: raise ValueError('six exact case4 journals required')
    return {'physical_rows':len(physical),'confirmed_commits':len(receipts),'confirmed_reports':sum(c['confirmed_reports'] for c in cohorts),'unreturned_operations':unreturned,'physical_commits_without_receipt':remaining,'emitted_errors':emitted_errors,'cohorts':cohorts}

def publish(dest, encoded):
    if dest.exists():
        if dest.read_bytes() != encoded: raise ValueError('different existing output refused')
    else:
        with dest.open('xb') as f: f.write(encoded)

def analyze(lab):
    result=reconcile(lab)
    prefix='private4-load-4'
    def record(name): return json.loads(read(lab/'logs'/name))
    activation=record(prefix+'-r1-activate.result.json')
    if activation['ExitCode'] != 0: raise ValueError('phase1 failed')
    returned=json.loads(activation['Stdout'])
    if not returned['executed'] or returned['activation']['no_op']: raise ValueError('not actual phase1')
    end=dt.datetime.fromisoformat(activation['ended_utc'].replace('Z','+00:00'))
    result['old_completed_after_phase1_return']=[{'label':c['label'],**o} for c in result['cohorts'] if '-old-' in c['label'] for o in c['operations'] if dt.datetime.fromisoformat(o['utc']) > end]
    result['phase1_return_utc']=activation['ended_utc']
    for c in result['cohorts']:
        expected='LIVE-f5-4' if '-new-' in c['label'] else 'LIVE-live1'
        if c['markers'] != [(expected,expected)] or c['confirmed_reports'] < 5: raise ValueError('cohort marker/report precondition')
    tails={}
    for name in ('refusal','noop'):
        proof=record(prefix+'-'+name+'-tail.json')
        if not proof['equal'] or proof['before'] != proof['after']: raise ValueError(name+' tail changed')
        tails[name]=proof
    if digest(lab/(prefix+'-r1.trn')).upper() != tails['noop']['after']: raise ValueError('current tail differs')
    storage={}
    for before,after in [('simple-before','simple-after'),('copyonly-before','copyonly-after'),('warm-before','warm-after'),('noop-before','noop-after')]:
        def rows(label):
            d={table:[] for table in ('Config','ConfigSave','Params','Files')}
            for line in read(lab/'snapshots'/(prefix+'-'+label+'-storage.txt')).splitlines():
                parts=line.split('|')
                if len(parts)==9 and parts[0] in d: d[parts[0]].append(line)
            if not d['Config'] or not d['Params']: raise ValueError('empty fullheader inventory')
            for table in d:
                if len({tuple(r.split('|')[1:3]) for r in d[table]}) != len(d[table]): raise ValueError('duplicate storage key')
            return {t:sorted(v) for t,v in d.items()}
        a,b=rows(before),rows(after)
        for table in ('Config','ConfigSave','Params'):
            if a[table] != b[table]: raise ValueError('storage changed '+before+' '+table)
        storage[before+'→'+after]={t:{'before_rows':len(a[t]),'after_rows':len(b[t]),'full_headers_and_data_sha_equal':a[t]==b[t]} for t in a}
    refusal=record(prefix+'-warm-refusal.result.json')
    if refusal['ExitCode'] != 1 or 'cycle 1 is retained, cycle 2 was not started' not in refusal['Stderr']: raise ValueError('warm refusal outcome')
    continued=json.loads(record(prefix+'-continue.result.json')['Stdout'])
    noop=json.loads(record(prefix+'-noop.result.json')['Stdout'])
    if continued['state']!='complete' or continued['cycle_2_executed'] is not True or noop['state']!='already_complete' or noop['cycle_2_executed'] is not False: raise ValueError('continue/noop outcome')
    token=returned['activation']['recovery_token']
    headercounts=[]
    for count in (1,2):
        rows=[r.split('|') for r in record(prefix+f'-header-{count}.result.json')['Stdout'].splitlines() if r.startswith('ibcmd-rs:live:')]
        if len(rows)!=count: raise ValueError('backup set count')
        for n,r in enumerate(rows,1):
            if r[0] != f'ibcmd-rs:live:{token}:{n}' or r[2]!='2' or r[5]!=str(n) or r[9]!='ibcmd_rs_05_load_w3_private4_20261001': raise ValueError('owned ordered HEADERONLY')
        if len(rows)==2 and rows[0][14]!=rows[1][13]: raise ValueError('log LSN chain')
        headercounts.append(count)
    reader=lab/'read_private4_recovery.py'
    if digest(reader).upper()!='51762747E7F46295B6A98786EAD9155EAD1FA4BCF9B9BA7A6F266C879F3587D7': raise ValueError('frozen reader changed')
    spec=importlib.util.spec_from_file_location('private4_frozen_reader',reader); module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    compact=module.verify(lab/(prefix+'-r1.recovery.json'),lab/(prefix+'-r1.recovery.live.json'),lab/'snapshots'/(prefix+'-staged-storage.txt'))
    if compact!=record(prefix+'-compact-readback.json'): raise ValueError('compact readback differs from runtime proof')
    cleanup=record('private4-lifetime-cleanup-status.json')
    if cleanup['resources_may_need_exact_owned_cleanup'] or not cleanup['guarded_stop_clean'] or cleanup['registration_state']!='unregistered' or cleanup['worker_lease_held_or_unconfirmed']: raise ValueError('cleanup not established')
    if list((lab/'private4-child-receipts').iterdir()): raise ValueError('uncertain child receipt retained')
    result.update({'scope':'case4 matching426 DEBUG loaded retained phase1/strict own reconnect; no warm readiness or native activation comparator', 'binary_source':'426c61731c73f242142d4d13a7b948ac83d97d66','script_source':'8babc8b9ab49d88b722a9768e6903b49f8505e56','tail_proofs':tails,'storage_comparisons':storage,'owned_header_counts':headercounts,'compact_readback':compact,'cleanup':cleanup,'raw_sha256':{str(p.relative_to(lab)):digest(p) for p in [lab/'snapshots/private4-physical-documents.txt',lab/(prefix+'-r1.trn'),*(lab/'obs').glob(prefix+'-*.log'),*(lab/'snapshots').glob(prefix+'-*-storage.txt'),*(lab/'logs').glob(prefix+'-*.json')]},'limits':['old unreturned calls remain unknown until final physical readback; absent document does not establish a returned report','old cohort stalled; no same-session refresh, zero-error, adaptive readiness or throughput claim','standalone compact recovery format2 and legacy LIVE format1 measured; compact LIVE format2 from later source455 was not used','UUID SQL byte mapping independently established against earlier1434 W2 BSP receipts; no extra COM readback here','Files may change operationally; equality is measured per snapshot pair, not assumed','no generic recovery undo or five loaded LIVE switches claimed']})
    return result

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('lab_root',type=Path);parser.add_argument('--output',type=Path,required=True);a=parser.parse_args()
    boundary=Path('F:/ibcmd/lab/05').resolve();lab=a.lab_root.resolve();dest=a.output.resolve()
    if not lab.is_relative_to(boundary) or not dest.is_relative_to(boundary): raise ValueError('F 0.5 lab boundary required')
    publish(dest,(json.dumps(analyze(lab),ensure_ascii=False,indent=2)+'\n').encode('utf-8'))
    print('PASS exact case4 physical receipts/unknowns, tails/fullheaders/ordered headers/compact readback; inputs read-only')
