# UT11 SQL acceptance (#359)

Status: **BLOCKED — no SQL acceptance run performed**. This runbook and the
offline collector do not close #359. CF/filedb parity and the historical
50,898-file result do not exercise the required SQL export/import paths.

## Inventory on the user's Mac, 2026-10-23

Read-only inspection under `/Volumes/Work/Dev/ibcmd-rs-lab` found:

- Repository master `f9ebb8dd6a4b84d57c72a72d5941931e48fae73b`; this work
  branches directly from it with no donor PR dependencies.
- `lab-env.sh` routes Cargo/Rustup/npm/TMP/XDG paths to the external lab.
- `/opt/1cv8/8.5.1.1592/ibcmd` exists. No `ragent` at that version directory.
  Presence alone does not establish native MSSQL support on this host.
- Docker CLI and daemon available; `docker ps -a` found six unrelated MCP
  services and no SQL Server container. No listener on local TCP port 1433.
  This does not rule out a remote or differently bound SQL service.
- `sqlcmd`, `tsql`, `psql`, Podman and Colima absent from PATH. SQL clients
  are optional for our direct TDS path; installing one would not supply a
  SQL service, authorized credentials or a UT11 SQL snapshot.
- External lab fixture/database directory metadata identifies empty/file
  databases and ERP copies; no UT11 SQL corpus was identified. No private
  ERP contents, credentials, connection configuration or production DB read.
- `docs/ssl-lab-2026-06-25.md` and `scripts/import-lab/` describe historical
  Windows localhost labs, not an authorized current endpoint for this Mac.
  `docs/evidence/export-timing-2214-20260824.md` identifies UT **11.5.27.75**
  from CF on macOS. Its numbers are historical, not current SQL evidence.

### Subsequent user-authorized template discovery

The user supplied the local `1c/trade` template directory. Read-only `.mft`
metadata identifies UT11 **11.5.27.68** and **11.5.27.88**, vendor Фирма 1С,
`AppVersion=8.3`; each contains a configuration `.cf` and demo `.dt`.
Only the newer version's CF and MFT were copied into a fresh private fixture
under the external lab; source and copy SHA-256 matched. The original was
not modified. Exact hashes and source path remain in the private local
`provenance.json`, outside Git. No private corpus, CF, XML or DT is published.
The metadata was not independently authenticated against a vendor signature.

This supplies an authorized **template**, not an existing UT11 SQL state.
A future lab may initialize a disposable SQL source from that CF only after
its endpoint and initialization operations are authorized; then freeze and
fingerprint the resulting SQL state for both exports. It does not satisfy
the missing SQL endpoint, source state or empty target requirements below.

No installer, server start, deployment, network change or SQL access was
attempted. Official client installation cannot remove the missing service
and corpus prerequisites. Findings shared with #336, #361 and #384 owners.

## Inputs required before a SQL run

The lab owner must identify these without publishing secrets:

1. Authorized disposable MSSQL endpoint and authentication mechanism, plus
   explicit source/target database identities and permitted operations.
2. UT11 SQL source snapshot: exact product/platform versions, provenance,
   immutable backup/snapshot identity and SHA-256, and authorization to use
   it. UT11 version may differ from historical 11.5.27.75; record the actual
   version. Do not substitute the ERP fixture or an unverified CF corpus.
3. A **distinct, natively initialized empty disposable SQL infobase** and
   authorization for our import and native apply/export there. Record the
   native create receipt and empty-state evidence before import; never
   clear an existing database to make it empty.
4. Matching native platform with a verified MSSQL route on the chosen host,
   license/auth prerequisites, and external-disk output paths. If a separate
   Windows lab is required, arrange it explicitly rather than interpreting
   old Windows paths as available access.
5. Source freeze/exclusivity procedure for both exports, timings and state
   fingerprints before/after. A database name or two matching trees alone
   does not prove identical SQL state.

Any server deployment or network change requires action-specific approval.
The original `/Users/jimmo910/1C/base_1C/ERP` is never a write target. Do not
search credential stores or infer authorization from historical connections.

## Reproducible execution once prerequisites exist

Coordinate native/full-suite capacity with the planner before running.
Create a fresh run directory outside the repository, under external lab
`fixtures/issue359-<run-id>`, with all logs/temp/native data there. Keep
`IBCMD_TEST_KEEP_FIXTURES=1`; no teardown deletion. Cleanup goes to Trash.

On the Mac, source `lab-env.sh`, set `CARGO_BUILD_JOBS=2`, use a dedicated
`CARGO_TARGET_DIR` under external `build/issue-359`, and explicitly select
external Rust 1.95's `RUSTC`/Cargo. Build the current selected master revision
and record full Git SHA, dirty status, binary SHA-256, toolchain, native
version/binary hash and host architecture. Refresh the pinned master for an
actual acceptance run: this document's base SHA is not a future run result.
If the binary runs elsewhere, record its target and build provenance too.

Before executing, resolve every placeholder below to reviewed disposable
identities. These are CLI shapes, not an executable provisioning script.
Use a secure supported auth mechanism (`-W`/environment for our tool); do
not place passwords in argv, shell history, logs, manifests or GitHub.
Validate native auth/platform flags with that exact platform's help.

1. Freeze the authorized UT11 SQL source. Capture the immutable source
   identity, SQL state fingerprints and freeze start in a local receipt.
2. Native full export from that source to fresh `native-source/`:
   `NATIVE infobase config export --dbms=MSSQLServer --db-server=LAB --db-name=SOURCE --data=EXTERNAL_NATIVE_DATA native-source`.
3. Our full export from the **same frozen source** to fresh `ours-source/`:
   `OURS infobase config export --dbms=MSSQLServer --db-server=LAB --db-name=SOURCE --report=EXTERNAL_REPORT ours-source`.
   Record both exit codes, UTC start/end, elapsed time and private logs.
   Capture source fingerprints again and verify unchanged before release.
   Any failure or source drift invalidates the run; retain failed artifacts.
4. Record distinct target identity, native create receipt and empty-state
   check. Import **ours-source/** into the empty target using default guard:
   `OURS infobase config import --dbms=MSSQLServer --db-server=LAB --db-name=EMPTY_TARGET --report=EXTERNAL_IMPORT_REPORT ours-source`.
   Record full verification report; no base-CF, guard bypass or acceptance
   based on a previous target's configuration. Stop on nonzero exit.
5. Complete the platform-required native apply (dynamic disabled) in that
   disposable target; record command/options and exit status. Then native
   full export target to fresh `native-after-import/`, recording timestamps,
   version, binary hash, exit code and resulting SQL state fingerprints.
6. Freeze the three output trees. Run the offline collector with an output
   outside them (all paths below inside the external run directory):

   ```sh
   python3 scripts/compare-sql-ut11.py \
     "$RUN/native-source" "$RUN/ours-source" "$RUN/native-after-import" \
     --output "$RUN/full-tree-comparison.json"
   ```

Collector exit: `0` both byte-exact comparisons match, `1` differences found,
`2` invalid/inaccessible/over-budget input or output. All three trees need
`Configuration.xml` and `ConfigDumpInfo.xml`. Defaults bound aggregate reads
at 600,000 files / 32 GiB; raise explicitly only after assessing corpus size.
No shell/SQL execution, network access, normalization or baseline update.
No overwrite or cleanup; symlinks/special files and overlapping roots fail.
Root and ancestor comparisons use physical directory identities (device and
inode), including aliases on case-insensitive filesystems. Inventories also
reject repeated physical directories within/across trees, covering mounted
subtree aliases whose backing ancestry is not visible in the input path.
Output containment uses physical parent/ancestor identities and the visited
directory identities, rather than pathname spelling alone. These checks
assume stable filesystem topology; they are not a sandbox against concurrent
mount/path replacement. Resolution failures, including older Python's
symlink-loop RuntimeError, use the same redacted exit-2 error path.
Input trees must remain immutable throughout hashing (the collector checks
file metadata during each read; it is not a SQL snapshot verifier).

The report stores complete path/size/SHA-256 inventories plus **every** missing,
extra or different file for native-source vs ours-source and native-source
vs native-after-import. `ConfigDumpInfo.xml` is included with no exception.
Any separately justified semantic analysis is supplemental; keep the full
byte diff and explicit remaining discrepancies. Do not publish this private
report or XML corpus. Share only approved aggregate counts and redacted
provenance; even paths/hashes may reveal private metadata.

`all_bytes_equal` describes trees only. The collector always labels SQL
acceptance as requiring independent provenance/run receipts. It cannot
verify SQL vs CF origin, product identity, native execution, source freeze,
empty target or a successful import. Review the receipts together with the
full diffs before accepting #359. Keep the issue open while any prerequisite
or discrepancy remains unresolved; do not use this Draft PR as completion.

## Focused collector checks

```sh
source /Volumes/Work/Dev/ibcmd-rs-lab/lab-env.sh
export IBCMD_TEST_KEEP_FIXTURES=1
export PYTHONDONTWRITEBYTECODE=1
export IBCMD_SQL_UT11_TEST_ROOT=/Volumes/Work/Dev/ibcmd-rs-lab/tmp/issue359-collector-tests
python3 tests/scripts/test_compare_sql_ut11.py
```

These synthetic tests retain fixtures and exercise equality, both diff legs,
missing/extra paths, version-file differences, budgets, aliases, symlinks,
partial trees, physical root/ancestor/mounted-subtree aliases, case aliases
where supported, output containment, symlink loops and no overwrite.
CI does not run them: the collector is used only in the #359 run, so run
them before it under Python 3.12 or older and 3.13 or newer to cover both
resolution-error behaviors.
All fixture directories are retained. They are collector
verification only and do not constitute SQL acceptance or native testing.
