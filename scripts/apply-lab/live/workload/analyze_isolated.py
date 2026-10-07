"""Read-only private83 workload/unknown-COMMIT reconciliation.

The SQL UUID layout was independently matched to all1434 earlier W2 COM
receipts on the same BSP schema. These samples do not prove warm admission.
"""
import argparse
import datetime as dt
import hashlib
import json
import re
from pathlib import Path

arguments = argparse.ArgumentParser(description=__doc__)
arguments.add_argument('lab_root')
arguments.add_argument('--output', help='fresh or byte-identical F 0.5 summary path')
options = arguments.parse_args()
lab = Path(options.lab_root).resolve()
boundary = Path('F:/ibcmd/lab/05').resolve()
dest = Path(options.output).resolve() if options.output else lab/'isolated-proof-summary.json'
if not lab.is_relative_to(boundary) or not dest.is_relative_to(boundary):
    raise ValueError('analysis output must stay in the F 0.5 lab')
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
physical = {}
for line in read(lab/'snapshots/isolated-physical-documents.txt').splitlines():
    cells = line.strip().split('|')
    if 'ibcmd-rs-load:' not in line: continue  # sqlcmd's database-context notice
    if len(cells) != 3 or not re.fullmatch('[0-9A-F]{32}', cells[0]):
        raise ValueError('malformed physical workload row')
    raw, posted, comment = cells
    if posted != '01': raise ValueError(f'unposted physical operation {comment}')
    uuid = f'{raw[24:32]}-{raw[20:24]}-{raw[16:20]}-{raw[:4]}-{raw[4:16]}'.lower()
    if comment in physical: raise ValueError(f'duplicate persisted operation {comment}')
    physical[comment] = {'uuid':uuid, 'posted':posted}

cohorts = []
receipts = {}
unreturned = []
emitted_errors = []
for path in sorted((lab/'obs').glob('isolated-*.log')):
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

native_start = json.loads(read(lab/'logs/isolated-native-apply-child.command.json'))['started_utc']
native_end = json.loads(read(lab/'logs/isolated-native-apply-child.result.json'))['ended_utc']
phase1_end = json.loads(read(lab/'logs/isolated-chain-3-r1-activate.result.json'))['ended_utc']
def at(value): return dt.datetime.fromisoformat(value.replace('Z','+00:00'))
native_during, native_after, own_after = [], [], []
for cohort in cohorts:
    for operation in cohort['operations']:
        item = {'label':cohort['label'], **operation}
        if cohort['label'].startswith('isolated-native-old-'):
            if at(native_start) <= at(operation['utc']) <= at(native_end): native_during.append(item)
            if at(operation['utc']) > at(native_end): native_after.append(item)
        if cohort['label'].startswith('isolated-chain-3-old-') and at(operation['utc']) > at(phase1_end): own_after.append(item)

for name in ('refusal','noop'):
    proof=json.loads(read(lab/f'logs/isolated-chain-3-{name}-tail.json'))
    if not proof['equal'] or proof['before'] != proof['after']: raise ValueError(f'{name} tail changed')
no_op=json.loads(read(lab/'logs/isolated-chain-3-noop-tail.json'))
if digest(lab/'isolated-chain-3-r1.trn').upper() != no_op['after']: raise ValueError('current tail differs from saved repeat SHA')
remaining = {c:v for c,v in physical.items() if c not in receipts}
unreturned_keys={f"ibcmd-rs-load:{o['label']}:{o['attempt']}" for o in unreturned}
if not set(remaining).issubset(unreturned_keys): raise ValueError(f'unexplained physical commits {set(remaining)-unreturned_keys}')
result={'scope':'isolated native disabled-dynamic and current3ff loaded checkpoint/owned reconnect, not warm admission',
 'physical_rows':len(physical),'confirmed_commits':len(receipts),'confirmed_reports':sum(c['confirmed_reports'] for c in cohorts),
 'unreturned_operations':unreturned,'physical_commits_without_receipt':remaining,'emitted_errors':emitted_errors,
 'native_child_utc':[native_start,native_end], 'native_completed_during_child':native_during,'native_completed_after_child':native_after,
 'own_phase1_return_utc':phase1_end,'own_old_completed_after_phase1_return':own_after,
 'cohorts':cohorts,'raw_sha256':{str(p.relative_to(lab)):digest(p) for p in [lab/'snapshots/isolated-physical-documents.txt',lab/'isolated-chain-3-r1.trn',*(lab/'obs').glob('isolated-*.log')]},
 'limits':['SQL UUID byte mapping inherited and independently established against all1434 W2 COM receipts on same schema; no additional COM readback here', 'physical unknown-commit reconciliation is final point-in-time readback only', 'DEBUG timing is not release performance','no generation is inferred from RAS activity/counters or idle SQL handles','script keyword/setup failures retained; first exit0 is not acceptance']}
encoded = json.dumps(result,ensure_ascii=False,indent=2).encode('utf-8')
try:
    with dest.open('xb') as output: output.write(encoded)
except FileExistsError:
    if dest.stat().st_size != len(encoded) or dest.read_bytes() != encoded:
        raise ValueError('existing summary differs; retained output preserved, choose a fresh --output')
print(json.dumps({k:result[k] for k in ('physical_rows','confirmed_commits','confirmed_reports','unreturned_operations','physical_commits_without_receipt')},ensure_ascii=False,indent=2))
print(f'native actual child overlap={len(native_during)} postreturnold={len(native_after)}; own postreturnold={len(own_after)}; emittederrors={len(emitted_errors)}')
