import os,sys,hashlib,collections
a,b=sys.argv[1],sys.argv[2]   # a=platform, b=ibcmd
def files(r):
    out={}
    for dp,dn,fn in os.walk(r):
        for f in fn:
            if f=='.complete': continue
            p=os.path.join(dp,f); out[os.path.relpath(p,r).replace(os.sep,'/')]=p
    return out
A,B=files(a),files(b)
onlyA=sorted(set(A)-set(B)); onlyB=sorted(set(B)-set(A)); both=sorted(set(A)&set(B))
eq=[p for p in both if open(A[p],'rb').read()==open(B[p],'rb').read()]
ne=[p for p in both if p not in set(eq)]
print('platform %d, ibcmd %d, common %d, identical %d (%.2f%%), differ %d, only-platform %d, only-ibcmd %d'%(len(A),len(B),len(both),len(eq),100*len(eq)/max(1,len(A)),len(ne),len(onlyA),len(onlyB)))
ext=lambda L: dict(collections.Counter(os.path.splitext(p)[1] for p in L))
print(' differ by ext:',ext(ne)); print(' only-platform by ext:',ext(onlyA)); print(' only-ibcmd by ext:',ext(onlyB))
for tag,L in (('only-platform',onlyA),('only-ibcmd',onlyB),('differ',ne)):
    for p in L[:12]: print('  ',tag,p)
bins=[p for p in A if p.endswith('.bin') and '/Ext/' in p and ('Module' in p)]
print(' module .bin in platform:',len(bins),' identical in ibcmd:',sum(1 for p in bins if p in B and open(A[p],'rb').read()==open(B[p],'rb').read()))
bsl=[p for p in A if p.endswith('.bsl')]
print(' .bsl in platform:',len(bsl),' identical:',sum(1 for p in bsl if p in B and open(A[p],'rb').read()==open(B[p],'rb').read()))
