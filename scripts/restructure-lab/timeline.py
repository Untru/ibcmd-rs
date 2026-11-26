"""Phase timeline of a traced native apply (events.jsonl): first/last time and count of marker statements."""
import json, re, sys
from datetime import datetime
ev=[json.loads(l) for l in open(sys.argv[1],encoding='utf-8')]
def ts(e): return datetime.fromisoformat(e['t'])
t0=ts(ev[0])
MARK=[
 ('read ConfigSave/Config rows (check metadata)', r'FROM ConfigSave WHERE FileName = @P1 ORDER BY PartNo'),
 ('copy ConfigSave -> Config.new', r'INSERT Config SELECT @P1'),
 ('SchemaStorage Status=200 + NewGenCreated', r'UPDATE SchemaStorage SET NewGenCreated'),
 ('create table ...NG', r'(?i)^\s*create table dbo\._\w+NG'),
 ('create index ...NG (1st pass)', r'(?i)^\s*CREATE (UNIQUE )?(CLUSTERED )?INDEX \w+NG'),
 ('drop index ...NG', r'(?i)^\s*drop index \w+NG'),
 ('copy data INSERT INTO ...NG', r'INSERT INTO dbo\._\w+NG WITH\(TABLOCK\)'),
 ('bulk copy (insert bulk ...NG)', r'insert bulk dbo\._\w+NG'),
 ('SchemaStorage Status=400', r'UPDATE SchemaStorage SET Status = 400'),
 ('drop table (old generation)', r'(?i)^\s*drop table dbo\._'),
 ('SchemaStorage Status=500', r'UPDATE SchemaStorage SET Status = 500'),
 ('sp_rename NG -> final', r'exec @P1 = sp_rename'),
 ('SchemaStorage Status=100 + CurrentSchema', r'UPDATE SchemaStorage SET Status = 100, CurrentSchema'),
 ('UPDATE DBSchema', r'UPDATE DBSchema SET SerializedData'),
 ('Params .sinew rename', r'UPDATE params SET FileName'),
 ('Config .new -> final rename', r'UPDATE config SET FileName'),
 ('DELETE FROM ConfigSave', r'DELETE FROM ConfigSave WHERE FileName LIKE'),
 ('Files help index rename', r'UPDATE Files SET FileName'),
]
def inner(t):
    m=re.match(r"(?is)^\s*exec\s+sp_executesql\s+N'((?:[^']|'')*)'",t or '')
    return m.group(1).replace("''","'") if m else (t or '')
rows=[]
for name,pat in MARK:
    hits=[e for e in ev if e['ev'] in ('rpc_completed','sql_batch_completed') and re.search(pat,inner(e.get('text')))]
    if not hits: rows.append((name,None,None,0)); continue
    rows.append((name,(ts(hits[0])-t0).total_seconds(),(ts(hits[-1])-t0).total_seconds(),len(hits)))
print('total span %.1fs, %d events'%((ts(ev[-1])-t0).total_seconds(),len(ev)))
for n,a,b,c in rows:
    print('%-46s %s'%(n, ('%7.1f .. %7.1f s  x%d'%(a,b,c)) if a is not None else '-'))
