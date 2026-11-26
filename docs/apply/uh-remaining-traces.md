# ERP УХ: remaining native apply traces (#336)

Status on 2026-10-23: **blocked, not measured**. This runbook and the synthetic
artifact-gate tests are preparation, not ERP УХ acceptance. Base: master
`f9ebb8dd`; no donor PR or stacked implementation is required.

Existing evidence stays valid: BSP 8.3.27 modules/attribute/new database,
BSP 8.5 modules, and ERP УХ 8.3.27 modules (native-apply-trace.md §§10–12).
Do not rerun them to fill the two missing cells. The old reduced scope excluded
ERP УХ cases 2/3; the reopened issue requires both. The 0.3 end-to-end result
cannot establish the native SQL write set.

## Prerequisites and current inventory

The external Mac lab has native `/opt/1cv8/8.5.1.1592/ibcmd`, PowerShell, and
file ERP fixtures. Neither those fixtures nor their name prove ERP **УХ**
identity/version or provide a SQL source. The #359 owner's read-only inventory
reported no SQL container, no TCP 1433 listener, and no authorized SQL endpoint.
No SQL connection, restore, import, apply, account creation or XE session was
attempted for #336. #361 shares that infrastructure prerequisite. #384 was
informed that module-only conclusions do not establish structural apply safety.

Before either run, the owner must supply and record locally:

- An explicitly authorized disposable SQL endpoint, its owner, existing auth
  mechanism, and permission for these exact clone/create/import/apply and XE
  operations. Do not discover passwords or create credentials. An unknown shared
  server is not an acceptable default. Server deployment/security/network changes
  are separate actions requiring specific authorization.
- A frozen ERP УХ SQL source with configuration name/version, platform version,
  backup SHA-256 and provenance. Keep the original ERP directory read-only.
- Two distinct disposable target identities (case 2 clone, case 3 fresh database),
  native data directories, approved server-side XE storage, free-space budget and
  a 60-minute command limit. Reserve capacity with the planner before running.
- A compatible native SQL platform, existing account and host. The saved kit
  currently uses **Windows integrated authentication** and a **Windows-local XE
  path** in `trace.ps1`. Mac pwsh plus Rosetta ibcmd does not establish this path.
  Run on an authorized compatible host, or review a separate portability change;
  do not silently substitute a file database or generate an SQL login.

All Mac-side outputs, caches and logs remain under
`/Volumes/Work/Dev/ibcmd-rs-lab`; source `lab-env.sh`, use the external Rust 1.95
cache/toolchain and explicit `RUSTC` if building ours (`CARGO_BUILD_JOBS=2`).
Never publish ERP CF/XML, row blobs, SQL statements containing private data, raw
XE, server names or credentials. Retain private evidence locally with hashes;
only reviewed aggregate findings belong in the PR.

## Capture contract

Reuse `scripts/apply-trace/`, not historical `lab/run_case3.ps1` verbatim: that
wrapper hardcodes another host and permanently deletes export output. No test
or capture cleanup may permanently delete files here. Use fresh output paths,
`IBCMD_TEST_KEEP_FIXTURES=1` and retain artifacts until moved to Trash. Always
pass **both `-KeepXel` and `-KeepRaw`**, because default trace cleanup deletes
XE and raw XML. `-KeepRaw` also retains the parsed intermediate TSV. Do not invoke wildcard `-Cleanup` on others' sessions.

On the authorized Windows capture host, set these variables to approved values
(no credentials in scripts or logs). The following commands are **templates,
not executed evidence**:

```powershell
$kit = '<checkout>\scripts\apply-trace'
$approvedServer = '<authorized disposable SQL endpoint>'
$caseDatabase = '<one approved disposable case database>'
$nativeExe = '<matching native ibcmd.exe>'
$caseData = '<fresh native data directory>'
$caseOutput = '<fresh private capture directory>'
$caseBlobs = '<fresh private blob store>'
$caseArgs = @('infobase','config','apply','--dbms=MSSQLServer',
  "--db-server=$approvedServer", "--db-name=$caseDatabase", "--data=$caseData",
  '--force','--dynamic=disable')
# Add the existing infobase user only if this fixture requires it.
& "$kit\capture.ps1" -Server $approvedServer -Database $caseDatabase `
  -Track uh336 -Tag '<uh-attribute|uh-newdb>-apply' `
  -OutRoot $caseOutput -BlobStore $caseBlobs -TimeoutMinutes 60 `
  -IncludeStatements -KeepXel -KeepRaw -DataChecksum full `
  -ContentMaxKB 32768 -ContentBudgetMB 4096 `
  -Exe $nativeExe -ArgumentList $caseArgs
# Pass the returned final capture directory, not OutRoot:
python "$kit\check_capture.py" '<capture directory>'
python "$kit\blobstore.py" verify $caseBlobs
```

The content limits are explicit budgets, not guarantees. Inspect snapshot metadata
and `blob=Y` coverage of every changed row; increase an approved budget or declare
missing payloads before claiming all writes. `RowDumpMaxRows=300000` and the default
4 GiB checksum scan cutoff may skip work: inspect every table note and all service
row counts. `check_capture.py` rejects skipped service dumps and unknown/nonzero
loss counters, but it checks artifact presence/identity only. It does not validate
compressed bytes, event counts/sequence, timestamps, SQL semantics, blob coverage,
checksums, source identity, XE rollover, or authorization. Exit 0 means only that
these artifact prerequisites are present; it never closes #336.

Record every failed or timed-out attempt separately. `capture.ps1` can write a
report after a trace failure, so a report directory is not proof of success.
The gate rejects failed commands. Preserve their evidence and capture any
approved retry with its own before/after pair; never overwrite the first result.

## Case 2: add an attribute

1. Restore a fresh approved clone of the frozen ERP УХ source. Record DB identity,
   restore provenance, baseline Config/ConfigSave/DBSchema/SchemaStorage hashes,
   table counts, extensions and change-register population. Confirm ConfigSave
   is empty and there are no unrelated sessions/writers.
2. Make a sparse private native XML edit: add exactly one new String(25) catalog
   attribute with a fresh recorded metadata UUID. Keep owner UUID and existing
   fields unchanged. Record owner, attribute, source/edited file hashes and the
   exact XML diff privately. Use native `config import files --partial` rather
   than our patch mode, which previously lost the attribute (§4).
3. Snapshot before staging and after staging; check the staged owner descriptor
   actually contains the attribute. A no-op or module-only stage is invalid.
   Trace staging separately if needed; do not mix its writes with apply.
4. Run the capture contract for exclusive apply. Verify the resulting physical
   column/type through schema diff and DBNames mapping. Native export plus full
   source comparison must demonstrate the attribute and absence of unrelated
   semantic changes. Save native check/export result codes and logs privately.

## Case 3: new database

1. Reserve a second never-used disposable name. Keep native create, our full
   import and native apply as three separately labelled operations; record exact
   binaries/commit hashes and commands. Do not restore the case 2 database here.
2. Capture native create with `trace.ps1` against existing `master`, using
   `-AllDatabases -IncludeStatements -KeepXel -KeepRaw -TimeoutMinutes 60` and the
   approved create argument list. This all-database capture is allowed only on an
   explicitly dedicated disposable server. The trace helper otherwise requires
   DB_ID to exist before starting. Its automatic focus on master may omit sessions that only touched the new
   database: regenerate the create report from raw XML with
   `trace_report.py --xml <raw> --meta <meta> --out <fresh-report> --keep-intermediate` **without
   `--focus-db`**, then reconcile master/new-DB sessions. On a shared server,
   stop and design an approved scoped collector; do not capture unrelated traffic.
3. Snapshot the natively initialized empty database, import the complete frozen
   reference using ours (record its JSON report and staged versions), and capture
   native apply using the contract above. The apply **before** snapshot must be
   after import, not the original empty database. Preserve the empty snapshot too.
4. Native export into a fresh directory, full source-diff against frozen reference,
   and native config check. Explain every difference (including configVersion).
   A successful export alone does not prove complete SQL capture. If first apply
   fails, retain it and authorize/record a separate retry; no automatic retry here.

## Independent analysis and acceptance

For **each** missing case, review and record:

- XE session SQL, capture window, original XEL hashes/files and rollover bounds;
  `dropped_events`, `dropped_buffers`, `largest_dropped` are measured zero. Check
  event sequence and exported count against raw events; explain any gaps, stop
  failure, truncation or rollover before asserting completeness. Do not infer zero
  from missing counters. Full statements can contain truncated binary parameters;
  resolve them using the before/after blob store, never invent the missing bytes.
- Before/after service tables: every row added/deleted/changed, stored and decoded
  hashes/sizes, schemas, DBNames, Config/ConfigSave/ConfigCAS/Params/Files,
  DBSchema/SchemaStorage, markers, help/search caches, `.ui`/`.si`, version ring,
  `_ConfigChngR`, `_DbCopies*`, extensions and all newly encountered families.
- Ordered phases and transaction IDs/boundaries, DDL, autocommit promotion and
  recovery markers. Reconcile transient writes absent from final row diffs with
  `service-writes.tsv`, `data-writes.tsv`, `ddl.sql`, `transactions.tsv`,
  `write-phases.md` and the raw events. Explain each native write family, including
  empty/no-write families; distinguish measured facts from hypotheses.
- Native exit codes, check/export/full-diff results and SHA-256 provenance for
  binaries, source and private evidence. An independent reviewer must compare the
  condensed public findings to the private complete capture. Send conclusions to
  #384; keep #359/#361 informed about the SQL prerequisite.

Only after both reviewed cases meet the original criterion may #336 be closed.
The separate harness PR can remain Draft while these external inputs are absent.
