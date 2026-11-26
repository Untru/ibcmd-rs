"""Compares the change-register tables of two twins without the random keys (_IDRRef): the rows of _ConfigChngR
(node, object), their _MessageNo, and the file lists in _ConfigChngR_ExtProps resolved through the key. The _MessageNo of
objects the stage does not touch is the documented drift of the long path (own-apply.md, "_MessageNo of objects the stage does not
touch"): the native rebuild writes 0 to the NULLs of the base, this apply leaves them; it is checked against the staged clone.
usage: python reg_cmp.py <native> <own> <staged-clone>"""
import sys
import pyodbc

CONN = "DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=master;Trusted_Connection=yes;TrustServerCertificate=yes"
nat, own, st = sys.argv[1:4]
cur = pyodbc.connect(CONN).cursor()


def keys(db, where):
    rows = cur.execute("SELECT _NodeTRef, _NodeRRef, _MDObjID FROM [%s].dbo._ConfigChngR WHERE %s" % (db, where)).fetchall()
    return {(bytes(r[0]), bytes(r[1]), bytes(r[2])) for r in rows}


all_nat, all_own, all_st = keys(nat, "1=1"), keys(own, "1=1"), keys(st, "1=1")
print("rows (node, object): native %d, own %d, staged clone %d; equal sets native/own: %s" % (
    len(all_nat), len(all_own), len(all_st), all_nat == all_own))
n_null, o_null, s_null = keys(nat, "_MessageNo IS NULL"), keys(own, "_MessageNo IS NULL"), keys(st, "_MessageNo IS NULL")
print("NULL _MessageNo: native %d, own %d, staged clone %d" % (len(n_null), len(o_null), len(s_null)))
print("reset or inserted by the apply (NULL now, not NULL before): native %d, own %d, equal: %s" % (
    len(n_null - s_null), len(o_null - s_null), (n_null - s_null) == (o_null - s_null)))
print("NULL in native, not in own: %d (must be 0)" % len(n_null - o_null))
print("NULL in own, not in native: %d, of them NULL in the staged clone already (the drift): %d" % (
    len(o_null - n_null), len((o_null - n_null) & s_null)))
E = ("SELECT r._NodeTRef, r._NodeRRef, r._MDObjID, e._KeyField, e._FileName FROM [%s].dbo._ConfigChngR r "
     "JOIN [%s].dbo._ConfigChngR_ExtProps e ON e._ConfigChngR_IDRRef = r._IDRRef")
ab = cur.execute("SELECT COUNT(*) FROM (%s EXCEPT %s) d" % (E % (nat, nat), E % (own, own))).fetchone()[0]
ba = cur.execute("SELECT COUNT(*) FROM (%s EXCEPT %s) d" % (E % (own, own), E % (nat, nat))).fetchone()[0]
n = cur.execute("SELECT COUNT(*) FROM (%s) d" % (E % (nat, nat))).fetchone()[0]
print("file lists (node, object, key, file): %d rows; only in native %d, only in own %d" % (n, ab, ba))
