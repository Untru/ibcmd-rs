# Main-configuration dynamic-update oracle, 2026-08-29

## Scope and safety

- Platform executable: `8.3.27.2214`.
- SQL database: disposable clone `ibcmd_rs_extlab_20260829` only.
- `IBVersion = 7`, `PlatformVersionReq = 80313`.
- `BSP_Service` was not written.
- Native `ibcmd` was used only as an oracle, with `--dynamic=force` and
  `--session-terminate=disable`.
- The experiment did not include an already-connected 1C session and therefore
  is not proof of online cache invalidation.

The initial read-only `mssql-activation-snapshot` attempt exposed an existing
tool bug: an empty `ConfigSave` causes `config_row_digests` to reject the
localized `sqlcmd` database-context line instead of accepting an empty JSON
array. The before-state was consequently captured with bounded explicit SQL
queries instead.

## Requested stage

The requested source object was `CommonModule._ДемоЗаметки`, metadata UUID
`ab132638-5188-470d-9432-de85f2b2c7d8`. A harmless trailing BSL comment was
added to the fixture. Existing `mssql-stage-common-module` produced five
`ConfigSave` rows:

| FileName | bytes | SHA-256 |
| --- | ---: | --- |
| `ab132638-5188-470d-9432-de85f2b2c7d8` | 123 | `3FAFDE9B31E6EBE61A4ABBA63A186DBC5A39F0292C3792BEE9E7E2F3849DCAC0` |
| `ab132638-5188-470d-9432-de85f2b2c7d8.0` | 1608 | `783F1C8EA6DF6A7F21E61BD9DE9E0D8F6E999A0987085B443D026B9E5BCF005F` |
| `root` | 45 | `ADFE2E91D2EFB05910281F6EFBB026ADFA876AA812D082C7F9E755028632A256` |
| `version` | 28 | `EC6D68D22CDFA563A8FA7B32B6C827ABC9B84B1C0FEBC543EFC70E450459B795` |
| `versions` | 344204 | `787FC81AC161DC48A686DDCAEA477EA32FEB5DA736BF56103437397585FE470D` |

The stage report selected internal configuration generation UUID
`8c2ac6ff-7309-4025-93f6-264cbb068d62`, replacing
`719baa18-69ed-439a-8962-1de53d98e05e`.

## Concurrency contamination

The shared clone was concurrently used by another isolated research task. That
task staged `b27aebc8-f190-4658-a81d-fd1406905f39` after the `ab132638...`
stage and before this task launched apply. Consequently this experiment is
contaminated and does **not** test the requested `_ДемоЗаметки` change or prove
any defect in UUID mapping.

Native dynamic apply completed and reported generation
`ffc62a8c0973254093f6264cbb068d6200000000`. `ConfigSave` was empty after the
operation. The SQL trace promoted `b27aebc8...`, which native
`ConfigDumpInfo.xml` identifies as
`CommonModule.ОплатаСервисаКлиентПереопределяемый`, and created these persistent
active rows:

- `b27aebc8-f190-4658-a81d-fd1406905f39`
- `b27aebc8-f190-4658-a81d-fd1406905f39.0`
- `b27aebc8-f190-4658-a81d-fd1406905f39_dynupdate_8c2ac6ff-7309-4025-93f6-264cbb068d62`
- `b27aebc8-f190-4658-a81d-fd1406905f39_dynupdate_8c2ac6ff-7309-4025-93f6-264cbb068d62.0`
- `versions_dynupdate_8c2ac6ff-7309-4025-93f6-264cbb068d62`

The target mismatch is explained by concurrent replacement of `ConfigSave`; it
must not be used as evidence about the existing `versions` patcher. A clean
experiment requires a separately named disposable clone held by one task.

## Observed dynamic-publication sequence

The useful, but auxiliary, oracle activity is session 137 around
`2026-08-29T08:45:31Z`. The trace shows a multi-phase protocol, not a direct
`ConfigSave -> Config` overwrite:

1. A UI/service row in `Params` is updated.
2. Five staged rows are copied to temporary `Config` names with `.new`.
3. Transient `Files.MobileVersions.datNEW`, `Config.commit`,
   `Config.dynamicCommit`, and `Config.dbStruFinal` rows are created/updated.
4. `Params.DynamicallyUpdated` is created. Its UTF-8-BOM text payload after
   apply is
   `{0,2,719baa18-69ed-439a-8962-1de53d98e05e,8c2ac6ff-7309-4025-93f6-264cbb068d62}`.
5. In a transaction the platform reads `_ConfigChngR` for the changed metadata
   identity and sets `_MessageNo = NULL` on five selected rows. No insertion or
   deletion in `_ConfigChngR` was seen in this cohort.
6. The `.new` rows are renamed into ordinary and generation-suffixed
   `_dynupdate_<generation>` rows. `root` and `version` replace their ordinary
   names; `versions` is retained as `versions_dynupdate_<generation>`.
7. `Config.DynamicallyUpdated` is inserted/updated with UTF-8-BOM text
   `{1,1,8c2ac6ff-7309-4025-93f6-264cbb068d62}`.
8. All five `ConfigSave` rows are deleted.
9. `MobileVersions.datNEW` replaces `MobileVersions.dat`; transient `commit`,
   `dynamicCommit`, and `dbStruFinal` rows are removed.
10. Two `.ui` rows in `Params` change and the dynamic markers remain.

Because the pre-apply five-row digest belongs to a different stage than the
applied `b27aebc8...` cohort, this experiment cannot compare staged and final
blob contents. It does establish that the native dynamic path retains ordinary
and generation-suffixed module rows, but a clean trace is needed before their
exact construction can be claimed.

## Exact service-state changes observed

Compared with the pre-stage `Params` digest:

- inserted `DynamicallyUpdated`, 82 bytes,
  SHA-256 `8719B886F007F9CE06D19939DCCDA2E20257F2BB3428A330ECA697AFA5C1F3C4`;
- changed `789702c6-d272-4008-9e55-db6f10687ae0.ui`, from 23477 to 24190 bytes;
- changed `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui`, 94 bytes with a new digest.

After apply, `Config.DynamicallyUpdated` is 45 bytes with SHA-256
`33C92FBED9177F489AD04028F7E39AB716C5E0612E7D289B6DE957C7D84FDB13`.

## What remains unevidenced

- A clean, non-concurrent before/stage/after cohort for one source identity.
- The binary grammars and semantic ownership of both changed `.ui` rows and
  `MobileVersions.dat`.
- Transaction boundaries for the complete operation: Extended Events proves
  several explicit subtransactions, but the full recovery/rollback state
  machine has not been reconstructed.
- Form publication.
- Exclusive publication.
- Extension publication.
- Any server cache-notification mechanism beyond the two
  `DynamicallyUpdated` rows.
- Observation by an already-running session without reconnecting.

## Verdict

Direct online activation is **not evidenced and must not be enabled**. The
trace exposes useful candidate service markers and the dynamic aliasing scheme,
but the shared-clone cohort is concurrency-contaminated, several binary service
artifacts remain opaque, and no live-session observation exists.

Primary artifacts:

- `main-module-after-stage-rows.txt`
- `main-module-after-apply-rows.txt`
- `params-before-stage.txt`
- `params-after-apply.txt`
- `xe-main-module-mutations.txt`
- `xe-main-module-events.txt` (raw, SQLCMD-wrapped diagnostic stream)
