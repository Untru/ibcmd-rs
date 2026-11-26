"""Diff of two DBSchema blobs at table/field/index level.
usage: python schemadiff.py <label1> <label2> [db]"""
import sys, difflib
import lab, bracefmt as bf
def tables(label, db=lab.DB):
    d=bf.parse(lab.dbschema(label, db))
    return {str(t[0]):t for t in d[1][1:]}, d
def main():
    l1,l2=sys.argv[1],sys.argv[2]
    db=sys.argv[3] if len(sys.argv)>3 else lab.DB
    t1,d1=tables(l1,db); t2,d2=tables(l2,db)
    print('header', bf.dumps(d1[0]), bf.dumps(d1[1][0]), '->', bf.dumps(d2[0]), bf.dumps(d2[1][0]))
    print('tables: %d -> %d'%(len(t1),len(t2)))
    print('order same for common:', [k for k in t1 if k in t2]==[k for k in t2 if k in t1])
    for k in t2:
        if k not in t1: print('ADDED table', k, 'at position', list(t2).index(k))
    for k in t1:
        if k not in t2: print('REMOVED table', k)
    for k in t1:
        if k in t2:
            a=bf.dumps(t1[k]); b=bf.dumps(t2[k])
            if a!=b:
                print('CHANGED table', k)
                # split by top-level fields for readable diff
                sa=a.replace('},{','},\n{').split('\n'); sb=b.replace('},{','},\n{').split('\n')
                for l in difflib.unified_diff(sa,sb,lineterm='',n=0):
                    if l.startswith(('---','+++')): continue
                    print('   ',l[:400])
main()
