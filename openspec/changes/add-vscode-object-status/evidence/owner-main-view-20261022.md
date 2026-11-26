# Canonical owners and SQL main view follow-up

Parent review found that the temporary mapper rejected ordinary ObjectModule/form
rows and made unrelated clean objects unknown. Dependency #362 a5fdd226 replaces
that mapper. Immediate/native/root owners remain in the RPC nativeOwners report;
supported nested changes roll up explicitly into the flat UI's owning object.
Storage UUID identity is not inferred from metadata UUID identity.
Repeat independent review also found an intermediate nested-subsystem case; the
fix rolls up only through shared-helper verified canonical ancestors with the
same root. A three-level A/B/C regression verifies that selecting B includes a
change in C. Repeat review found no further confirmed P1/P2.

Unit regressions cover Catalog.C.ObjectModule, Document.D.ObjectModule, a body-only
Catalog.C.Form.F.Form with an independent storage UUID and CommonModule.Unrelated.
Identical inventories are clean; mixed module/form changes affect only their
intended objects. Truly unknown roles remain inconclusive.

Native 8.5.1.1592 run (release profile 8.5.1):
`tmp/issue-380-native-owners/issue380-owners-fb72fe9023` under the external lab.
Only committed public form-table-representation-matrix CF and tiny module XML were
used, in a newly created disposable database. Native commands always specified
its db-path/data/temp. Full command/stdout/stderr/RPC evidence is retained there.
Identical native status was empty; all six top-level RPC objects were conclusive
and unchanged. Selected native import changed Catalog.CorpusList.ObjectModule and
its ListForm module. Native export status reported Catalog.CorpusList, its form,
form body and ObjectModule; RPC changed only Catalog.CorpusList. Unrelated common
module, other catalog, report, language and configuration stayed unchanged.
Both RPC phases verified the 1CD SHA before/after. Reproduction script:
`editors/vscode/test/prepare-native-owners.py`. This is platform command evidence,
not a Configurator GUI claim. Document mapping is covered by the unit regression.

SQL support is implemented, not intentionally refused. A separate main-view cache
reuses the shared read-only CONFIGSAVE overlay for tree and bytes as well as the
native version reader. Whole main export precedes object-file partitioning for SQL,
since selected raw rows cannot reconstruct completed staging. Legacy API defaults
stay active. Recording SqlClient tests verify CONFIG and CONFIGSAVE SELECT routing,
main export arguments and cache isolation; every write/script API panics. No live
SQL server or credentials were available or tested. Absence of version evidence
never falls back to byte-only clean. Full SQL performance belongs to #379.
