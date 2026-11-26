import sys, zlib
import lab, names
def ents(label):
    t=zlib.decompress(lab.row(label,'Params','DBNames'),-15)
    maxn,count,e=names.parse_dbnames(t)
    return maxn,count,e
a,b=sys.argv[1],sys.argv[2]
ma,ca,ea=ents(a); mb,cb,eb=ents(b)
print('header max %d -> %d ; count %d -> %d'%(ma,mb,ca,cb))
sa=set(ea); sb=set(eb)
nm=names.dumpinfo_names(lab.NATIVE_TREE + '\\ConfigDumpInfo.xml')
import re
# names of the new lab uuids from the patches
extra={}
for line in sys.argv[3:]:
    u,n=line.split('=',1); extra[u]=n
for u,k,n in sorted(sb-sa, key=lambda x:x[2]):
    print('ADDED',n,k,u,nm.get(u) or extra.get(u,'?'))
for u,k,n in sorted(sa-sb, key=lambda x:x[2]):
    print('REMOVED',n,k,u,nm.get(u,'?'))
print('order kept (common entries):',[e for e in ea if e in sb]==[e for e in eb if e in sa])
