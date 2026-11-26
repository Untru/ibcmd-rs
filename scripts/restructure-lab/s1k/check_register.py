"""The change register (`_ConfigChngR`, `_ConfigChngR_ExtProps`) of two twins, without the random keys (`_IDRRef`): the rows (node, object),
their `_MessageNo`, and the file lists resolved through the key. After the trace track's `reg_cmp.py` (S1-F, docs/apply/new-object.md 8),
without its third argument (the staged clone), so the drift of `_MessageNo` is reported, not proved away.

usage: python check_register.py <native db> <own db>

Equal: the sets of (node, object) rows and the file lists. Known drift (own-apply.md): the native rebuild writes 0 to the NULL `_MessageNo`
of the objects the stage does not touch, this apply leaves them NULL; so "NULL in native, not in own" must be 0, and "NULL in own, not in
native" is the drift (every one NULL in the staged clone already).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import db as dbm  # noqa: E402

nat, own = sys.argv[1:3]


def keys(db, where):
    rows = dbm.rows(db, "SELECT _NodeTRef, _NodeRRef, _MDObjID FROM dbo._ConfigChngR WHERE %s" % where)
    return {(bytes(r[0]), bytes(r[1]), bytes(r[2])) for r in rows}


all_nat, all_own = keys(nat, "1=1"), keys(own, "1=1")
print("R. change register rows (node, object): native %d, own %d; equal sets: %s" % (len(all_nat), len(all_own), all_nat == all_own))
n_null, o_null = keys(nat, "_MessageNo IS NULL"), keys(own, "_MessageNo IS NULL")
print("R. NULL _MessageNo: native %d, own %d; NULL in native and not in own: %d (must be 0); NULL in own and not in native: %d (the drift)"
      % (len(n_null), len(o_null), len(n_null - o_null), len(o_null - n_null)))
E = ("SELECT r._NodeTRef, r._NodeRRef, r._MDObjID, e._KeyField, e._FileName FROM [%s].dbo._ConfigChngR r "
     "JOIN [%s].dbo._ConfigChngR_ExtProps e ON e._ConfigChngR_IDRRef = r._IDRRef")
ab = dbm.rows(nat, "SELECT COUNT(*) FROM (%s EXCEPT %s) d" % (E % (nat, nat), E % (own, own)))[0][0]
ba = dbm.rows(nat, "SELECT COUNT(*) FROM (%s EXCEPT %s) d" % (E % (own, own), E % (nat, nat)))[0][0]
n = dbm.rows(nat, "SELECT COUNT(*) FROM (%s) d" % (E % (nat, nat)))[0][0]
print("R. file lists (node, object, key, file): %d rows; only in native %d, only in own %d" % (n, ab, ba))
