"""Ordered structure-phase statements of a traced apply (from xe/<case>/events.jsonl) as a readable SQL log.
usage: python extract_ddl.py <case> > file"""
import json, re, sys
import lab
case=sys.argv[1]
ev=[json.loads(l) for l in open(lab.ROOT + '\\xe\\%s\\events.jsonl' % case, encoding='utf-8')]
def inner(t):
    m=re.match(r"(?is)^\s*exec\s+sp_executesql\s+N'((?:[^']|'')*)'(.*)$",t or '',re.S)
    if m: return m.group(1).replace("''","'"), m.group(2)
    m=re.search(r"(?is)exec\s+sp_prepexec\s+@p1\s+output\s*,\s*N'(?:[^']|'')*'\s*,\s*N'((?:[^']|'')*)'\s*,@p4\s+output\s*,\s*(.*?)\s+select @p1",t or '',re.S)
    if m: return m.group(1).replace("''","'"), m.group(2)
    return t or '', ''
KEEP=re.compile(r'(?is)^\s*(create table dbo\._\w+NG|CREATE (UNIQUE )?(CLUSTERED )?INDEX \w+NG|drop index \w+NG|drop table dbo\._|INSERT INTO dbo\._\w+NG|insert bulk dbo\._\w+NG|UPDATE SchemaStorage|UPDATE DBSchema|exec @P1 = sp_rename|ALTER INDEX|BEGIN TRAN|COMMIT TRAN)')
out=[]; started=False
for e in ev:
    if e['ev'] not in ('sql_batch_completed','rpc_completed'): continue
    sql,args=inner(e.get('text'))
    if not KEEP.match(sql): continue
    if sql.upper().startswith(('BEGIN TRAN','SET TRANSACTION','COMMIT')): continue
    started=True
    hdr='-- %s sid=%s %s rows=%s dur=%sus'%(e['t'][11:26],e.get('sid'),e['ev'],e.get('row_count'),e.get('duration'))
    if args:
        a=re.sub(r'0x[0-9A-Fa-f]{60,}',lambda m:'0x<%d bytes>'%((len(m.group(0))-2)//2),args.strip())
        hdr+='\n-- args: '+a[:400]
    out.append(hdr+'\n'+sql.strip().rstrip(';')+';\n')
print('\n'.join(out))
