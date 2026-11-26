"""Copies snapshot.py and db.py of the ddl kit into the trace kit with another store (F:\\ibcmd\\lab\\05\\s1g\\store)."""
src = r'F:\ibcmd\lab\04\restructure\tools'
dst = r'F:\ibcmd\lab\05\s1g\kit'
s = open(src + '\\snapshot.py', encoding='utf-8').read()
s = s.replace('ROOT = r"F:\\ibcmd\\lab\\04\\restructure"', 'ROOT = r"F:\\ibcmd\\lab\\05\\s1g\\store"')
s = s.replace('F:\\\\ibcmd\\\\lab\\\\04\\\\restructure\\\\snap', 'F:\\\\ibcmd\\\\lab\\\\05\\\\s1g\\\\store\\\\snap')
assert 'lab\\05\\s1g\\store' in s
open(dst + '\\snapshot.py', 'w', encoding='utf-8', newline='\n').write(s)
d = open(src + '\\db.py', encoding='utf-8').read()
open(dst + '\\db.py', 'w', encoding='utf-8', newline='\n').write(d)
print('ok')
