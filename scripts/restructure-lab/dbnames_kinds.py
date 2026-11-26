"""Association of metadata classes (ConfigDumpInfo names) to the DBNames kinds allocated for them."""
import collections, zlib, sys
import lab, names
label=sys.argv[1] if len(sys.argv)>1 else 's01_after_baseline_noop'
t=zlib.decompress(lab.row(label,'Params','DBNames'),-15)
maxn,count,ents=names.parse_dbnames(t)
nm=names.dumpinfo_names(lab.NATIVE_TREE+r'\ConfigDumpInfo.xml')
by=collections.defaultdict(list)
for u,k,n in ents: by[u].append((k,n))
def cls(name):
    parts=name.split('.'); out=[parts[0]]; i=2
    while i<len(parts): out.append(parts[i]); i+=2
    return '.'.join(out)
c=collections.defaultdict(collections.Counter)
for u,lst in by.items():
    name=nm.get(u)
    key=cls(name) if name else ('<nil uuid: platform tables>' if u=='00000000-0000-0000-0000-000000000000' else '<not in ConfigDumpInfo>')
    c[key][tuple(sorted(k for k,_ in lst))]+=1
print('DBNames of the 8.3.27 БСП main configuration: max %d, entries %d'%(maxn,count))
print('metadata class -> number of objects, and the sets of DBNames kinds allocated for them')
for key in sorted(c, key=lambda k:-sum(c[k].values())):
    tot=sum(c[key].values())
    if key.startswith('<nil'):
        print('%-46s %5d  (%d distinct kinds, e.g. DbCopies*, ConfigChngR, ExtensionsInfo, STT*, ...)'%(key,tot,len(next(iter(c[key])))))
        continue
    print('%-46s %5d  %s'%(key,tot,dict(c[key].most_common(4))))
