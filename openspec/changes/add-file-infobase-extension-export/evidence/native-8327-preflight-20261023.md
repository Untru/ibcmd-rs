# Native file extension export preflight, 2026-10-23

## Tools and scope

macOS, native platform **8.3.27.2214**, candidate baseline
`1be98ec77a8096553915d555cdd5ffb10fe34e99`. Native binary SHA-256:

- ibcmd: `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`.
- Designer: `6e45dc373585a2f60c3ae680e981c5011cc681c38ccffe84a64156cef3002da8`.

Only disposable clean-room file infobases were opened. Source templates are
`_onecdec/fixture_src/cfe_base` and `ТестРасширение`. A second case adds a
common module to the base and adopts it in the extension. A second own
extension has independent identities; its adopted language retains the base
language identity. No EDT, SQL server or application corpus is involved.

## Commands and observations

Designer CREATEINFOBASE, LoadConfigFromFiles, UpdateDBCfg, extension
LoadConfigFromFiles and DumpConfigToFiles -Extension all succeeded on the
minimal case. Native ibcmd:

```text
ibcmd infobase config export --db-path=<disposable-db> --data=<unique-work-dir> --extension=ТестРасширение <new-output>
```

Designer and ibcmd produced **5/5 identical files**, **7637 bytes**, including
ConfigDumpInfo.xml. No normalization or exclusions. Both tree SHA-256 values:
`d22d75a2ef8021a2fa59eb71a7b55078e12d08dbbce023238ac1d93c8bc0d19a`.
Tree hash encodes sorted UTF-8
`path + NUL + lowercase-file-SHA256 + NUL + decimal-size + LF`.

Missing extension: rc 1, stderr
`[ERROR] Операция невозможна: расширение ОтсутствующееРасширение не найдено`;
no output directory. The database SHA-256 was unchanged through both ibcmd
operations.

The two-extension case was exported independently before extension apply
and after explicit `Designer /UpdateDBCfg -Extension <name>` for each:

| state | extension | rc | exported files after explicit apply |
| --- | --- | --- | --- |
| prepared | own + adopted common module | 0 | — |
| prepared | second own extension | 0 | — |
| applied | own + adopted common module | 0 | 6 / 9431 bytes |
| applied | second own extension | 0 | 5 / 7669 bytes |

Every measured ibcmd export retained the exact database SHA-256. Merely
running UpdateDBCfg for the main configuration did **not** apply the
extensions: CONFIGCAS remained empty. An early second-extension source had
an incorrectly remapped adopted-language reference; its apply failed with
rc 101. The reference was corrected and both explicit extension applies
succeeded. These failed/intermediate attempts are not native acceptance.

## Measured physical profile

The file format is 8.3.8; the observed page size is 8192. Registry table
`_EXTENSIONSINFO` declares these nonnullable fields, in this schema order:

| name | kind | length |
| --- | --- | --- |
| _IDRREF | B | 16 |
| _EXTENSIONORDER | N | 9 |
| _EXTNAME | NVC, CI | 255 |
| _UPDATETIME | DT | 0 |
| _EXTENSIONUSEPURPOSE | N | 2 |
| _EXTENSIONSCOPE | N | 2 |
| _EXTENSIONZIPPEDINFO | I | 0 |
| _MASTERNODE | NT, CI | 0 |
| _USEDINDISTRIBUTEDINFOBASE | L | 0 |
| _VERSION | RV | 0 |

The record size is **578 bytes**. Unlike schema order, the physical RV
occupies the first 16 data bytes after the free flag, followed by the other
fields in schema order. Field locations must be derived from the validated
profile; treating RV as the last physical field misreads the registry id
and name.

The zipped-info magic is `43 c2 9a 14`; the next 20 bytes are the active CAS
root, zero before first apply. The observed file envelope starts
`a2 9a 08`, unlike the existing SQL decoder's `a1 9a 08` profile. This must
be an explicit measured file profile, not a relaxation of SQL parsing.

`CONFIGCASSAVE` uses `<registry-namespace-uuid>__<logical-name>` keys,
including `configinfo`. Physical _IDRREF bytes map to that namespace using
the existing extension_namespace_prefix permutation. Before first apply,
the minimal extension has five live prepared rows and no active CAS rows.
After explicit apply, it has five SHA-1-keyed CONFIGCAS rows and no prepared
rows. In the applied two-extension case, CONFIGCAS has eleven live rows.
The `_EXTENSIONSINFONGS` table uses the same declared registry fields and
has no live records in these samples. Other NGS states are unmeasured.

## Baseline before implementation

The unmodified candidate's public command against the same two-extension
database returns rc 255:
`Выгрузка расширений из файловой информационной базы пока не поддерживается`.
The root file_infobase integration harness compiled successfully before the
change. Final candidate parity and gate results are recorded in
`verification-20261023.md`.

## Implementation acceptance

The public regression first failed with the unsupported-route error and rc
255, both for native-tree export and the missing-name code-1 expectation.
After implementing the route, exact native per-file SHA-256 comparisons pass
for both extensions in three snapshots: initial prepared, applied, and mixed
(an applied first extension is edited and prepared again while the second
stays active). The modified module is selected from staging. Native mixed
exports both returned rc 0 and left the database hash unchanged.

Native name probes additionally accept Cyrillic lowercase and trailing ASCII
spaces; each produces the same output as the stored spelling. The candidate
regressions exercise both.

The portable tests compare every file, including ConfigDumpInfo.xml, with no
normalization. They assert the database's whole byte vector remains unchanged
after each candidate export. Missing names, invalid registry layout/envelope,
missing or changed CAS content, incomplete/unclassified staging, incremental
options, nonempty outputs and output symlinks have refusal tests. A crate
test injects a live next-generation registry view and checks refusal.

Minimal clean-room gzip snapshots and native file hashes are retained in
`tests/fixtures/file-infobase/extensions-8327/`. Raw journals and full native
XML trees remain outside the repository. Strict OpenSpec, filedb clippy and
`cargo xtask check` pass; final ci and independent review are recorded in the
change's verification evidence when complete. This evidence does not claim
private corpora, SQL, online activation, 8.5 or unsupported next-generation
selection.
