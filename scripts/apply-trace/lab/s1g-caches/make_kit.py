import re
src = r'F:\ibcmd\lab\04\restructure\tools'
dst = r'F:\ibcmd\lab\05\s1g\kit'


def rd(name):
    return open(src + '\\' + name, encoding='utf-8-sig').read()


def wr(name, text):
    open(dst + '\\' + name, 'w', encoding='utf-8-sig', newline='\r\n').write(text)


DB_OLD = r'ibcmd_rs_04_ddl_[a-z0-9_]+'
DB_NEW = r'ibcmd_rs_05_trace_[a-z0-9_]+'

s = rd('params_row.ps1')
s = s.replace(DB_OLD, DB_NEW).replace('ibcmd_rs_04_ddl_*', 'ibcmd_rs_05_trace_*')
s = s.replace('ddl-track lab database', 'trace-track lab database (copy of the ddl kit, other database prefix)')
wr('dbrow.ps1', s)

s = rd('srv.ps1')
s = s.replace(DB_OLD, DB_NEW).replace('ibcmd_rs_04_ddl_*', 'ibcmd_rs_05_trace_*').replace('not a ddl-track database', 'not a trace-track database')
s = s.replace("$root = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\\ibcmd\\lab\\04\\restructure' }", "$root = 'F:\\ibcmd\\lab\\05\\s1g\\kit'")
s = s.replace('[int]$Port = 5514', '[int]$Port = 5614')
wr('srv.ps1', s)

s = rd('session_job.ps1')
s = s.replace("$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\\ibcmd\\lab\\04\\restructure' }", "$lab = 'F:\\ibcmd\\lab\\05\\s1g\\kit'")
s = s.replace('[int]$Port = 5514', '[int]$Port = 5614')
s = s.replace('if (-not $Epf) { $Epf = "$lab\\probe\\ddl_probe.epf" }', "if (-not $Epf) { $Epf = 'F:\\ibcmd\\lab\\04\\restructure\\probe\\ddl_probe.epf' }")
wr('job.ps1', s)

for f in ('dbrow.ps1', 'srv.ps1', 'job.ps1'):
    t = open(dst + '\\' + f, encoding='utf-8-sig').read()
    print(f, [l.strip() for l in t.splitlines() if 'ddl' in l.lower() or '5614' in l or 'kit' in l][:8])
