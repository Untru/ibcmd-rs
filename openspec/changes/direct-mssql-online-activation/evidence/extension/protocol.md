# Extension publication oracle: MSSQL / 8.3.27.2214

## Scope and laboratory

- Source database: `BSP_Service` (read only).
- Disposable oracle clone: `ibcmd_rs_activation_ext_20260829`.
- Platform oracle: native `ibcmd` 8.3.27.2214, used only against the clone.
- Selected active extension: `_ДемоРасширение`.
- Changed body: existing common-module target
  `cd9aecaa-e480-481c-8c77-ff6600837681.0`; the only semantic change was one
  BSL comment.
- XE captures:
  `D:\SQL\ibcmd_rs_activation_ext_oracle_0_134324671114660000.xel` and
  `D:\SQL\ibcmd_rs_activation_ext_oracle_active2_0_134324677383010000.xel`.
  The server XE session was stopped and dropped after capture. The clone and
  `.xel` file were retained for review.

The active `configinfo` mapped 169 logical content rows. The oracle staging
contained those 169 rows plus namespaced `configinfo` (170 rows, 209601 packed
bytes). The active and proposed roots were:

```text
before root  35571124d08b1ad5dbf127247fd18626c615134b
before body  7d5be1f3f72ab82a086d1a0811f54fc67b5e363b
after body   2ffb528fa36bb1868563ba8ba5c9a56d3d48ddf8
after root   4a0957329560d104a0209ccca19555e642983a75
```

The first pass started from the BSP demo's `flag=1` pending state. A second
comment change was then applied from the resulting `flag=0` published state,
which produced:

```text
pass-2 before root  4a0957329560d104a0209ccca19555e642983a75
pass-2 before body  2ffb528fa36bb1868563ba8ba5c9a56d3d48ddf8
pass-2 after body   ed073838e716fce9e939f176be93e4a1c8f076b0
pass-2 after root   7f3c4b001849c06a7d457fb83557d9b40f616579
```

Native apply reported exactly each proposed root. After apply, both proposed
packed rows existed in `ConfigCAS`, all 170 selected staging rows were gone,
and the old root/content remained addressable.

## Registry transition

Before:

```text
_Version    000000000000185f
_UpdateTime 4026-03-16T22:48:24
root        35571124d08b1ad5dbf127247fd18626c615134b
blob        43c29a1435571124d08b1ad5dbf127247fd18626c615134ba19a080000000100000001818197487b002200230022002c00380037003000320034003700330038002d0066006300320061002d0034003400330036002d0061006400610031002d006400660037003900640033003900350063003400320034002c000a007b0031002c0022007200750022002c00220020043004410448043804400435043d043804350422007d000a007d009a08312e302e312e3134828120
```

After:

```text
_Version    0000000000017ed2
_UpdateTime 4026-08-29T11:54:33
root        4a0957329560d104a0209ccca19555e642983a75
blob        43c29a144a0957329560d104a0209ccca19555e642983a75a19a08000000000000185f818197487b002200230022002c00380037003000320034003700330038002d0066006300320061002d0034003400330036002d0061006400610031002d006400660037003900640033003900350063003400320034002c000a007b0031002c0022007200750022002c00220020043004410448043804400435043d043804350422007d000a007d009a08312e302e312e3134828120
```

The blobs have the same 184-byte length. Only these offsets changed (zero
based):

- `[4..24)`: the 20 raw SHA-1 root bytes;
- byte `30`: `01 -> 00` (published-generation marker);
- `[31..35)`: `00000001 -> 0000185f`.

The second, published-to-published transition changed `_Version`
`0000000000017ed2 -> 0000000000017ed3`, retained marker byte 30 as `00`, and
changed `[31..35)` from `0000185f` to `00017ed2`. Thus the new four-byte field
is exactly the low 32 bits of the *expected previous*
SQL `_Version` (`000000000000185f`), in big-endian order. This resolves the
previous inactive-to-active ambiguity: that earlier successful oracle stored
the row version produced before its successful retry. It is not `@@DBTS` at
the time of the registry update.

No other `_ExtensionZippedInfo` byte changed. Native updated all ordinary
registry columns to their existing values, set `_UpdateTime` to the 1C
date-domain timestamp (`wall clock + 2000 years`), supplied the patched blob,
and let SQL Server generate a new `_Version`. The exact RPC and parameters can
be reproduced with `registry_rpc.sql`.

This means a bounded registry encoder can be safe for the already validated
8.3.27 envelope: preserve the entire snapshot, replace only the three regions
above, and fail closed unless the blob decoder proves the exact evidenced
shape. An optimistic predicate must include `_IDRRef`, the full expected
`_Version`, and the full expected blob.

## CAS promotion

Native did not insert directly under the final hash. It used temporary names:

```text
2ffb...ddf8.new347eea02-0773-11e8-8780-107b44a2858a
4a09...3a75.new347eea02-0773-11e8-8780-107b44a2858a
```

It wrote/validated those rows, deleted the final name only when the temporary
name existed, and renamed temporary names to the final lower-case SHA-1. Exact
rename RPCs are in `cas_rename_rpc.sql`. A direct implementation need not copy
this temporary-name choreography if one serializable transaction instead:

1. computes SHA-1 over the exact packed bytes;
2. inserts a missing `(FileName = lower SHA-1, PartNo = 0)` row;
3. for an existing hash, verifies the complete row image and refuses any hash
   collision/different bytes;
4. publishes the registry only after every mapped row and the root are
   present.

Native final rows used `Attributes=0`, `PartNo=0`, `DataSize=DATALENGTH`, and
the 1C date-domain timestamp. It retained old CAS rows. Therefore newly
inserted content is recoverable orphan data if publication later aborts or is
rolled back.

Native also created/updated a selected-prefix service row:

```text
dbStruFinal347eea02-0773-11e8-8780-107b44a2858a
Attributes=0, PartNo=0, DataSize=0, BinaryData=empty
```

It was absent before and present after. This extension contains database
objects even though the selected edit was module-only. The service row is not
content-addressed and must not be covered by the immutable-hash rule. On the
second published-to-published pass its `Creation` remained
`4026-08-29T11:54:33`, `Modified` advanced to `4026-08-29T12:03:12`, and its
empty payload and all other fields remained exact.

## Staging cleanup

Native finally deleted all 170 rows with a broad
`FileName LIKE '347eea02-0773-11e8-8780-107b44a2858a%'` predicate. The product
implementation should use the stricter escaped namespace
`347eea02-...__%`, hold the prefix range lock, and verify the exact expected
row count and aggregate byte count before deletion.

## Service/cache observations

- `DBSchema` and `SchemaStorage` were unchanged.
- `_ExtensionsInfoNGS` stayed empty.
- `_ExtensionsRestruct` was rebuilt transiently but its final row set was
  byte-identical to the source clone.
- `_ExtensionsRestructNGS` changed from a zero-byte `-1` sentinel to a 444-byte
  UTF-8 record about the unrelated `ServiceDesk` extension. This is global
  pending/restructuring state, not a deterministic function of the selected
  module or its CAS root. Publication must not guess or synthesize it. A first
  exclusive implementation should preflight this state and either preserve a
  recognized quiescent image or fail closed until a second oracle isolates its
  lifecycle. On the second published-to-published pass this row stayed
  byte-identical, proving it is not part of the routine selected-module root
  switch.
- Two `.ui` rows in `Params` changed (`789702c6-...ui` and
  `97f2c291-...ui`). Native also rebuilt help/postings/vocabulary rows in
  `Files`, keyed by the new root. These are derived caches, written after the
  authoritative root switch in separate transactions.
- This clean extension oracle did **not** write a `DynamicallyUpdated` row in
  either `Config` or `Params`. Such rows observed in the shared clone belonged
  to a concurrent main-configuration oracle and are not extension evidence.

The statement census is reproducible with `summarize_xe.sql`; `read_xe.sql`
shows the full captured statements.

## Minimal exclusive transaction supported by this evidence

The core authoritative transition can be implemented as one application-owned
serializable transaction, stricter than native's multiple autocommit steps:

1. acquire a database-scoped application lock plus row/range locks on the
   selected registry row, its escaped staging prefix, selected final CAS
   hashes, and `dbStruFinal<uuid>`;
2. recheck exact registry `_Version` and full blob, exact active root, exact
   staged manifest/mappings/count/bytes, and the known storage profile;
3. insert missing immutable CAS content/root rows and verify byte-identical
   existing rows;
4. create or refresh the evidenced empty `dbStruFinal<uuid>` row only when the
   activation plan explicitly classifies that service marker as required;
5. patch the registry root, marker and previous-rowversion field, set the 1C
   timestamp, and require exactly one updated row;
6. delete exactly the selected escaped staging prefix;
7. verify the new root resolves completely, the registry blob decodes to it,
   the staging prefix is empty, and no unrelated registry row changed;
8. commit.

Unknown service-row/restructuring state must abort before step 3.

## Recovery and blockers

- A transaction failure before commit needs no recovery artifact beyond the
  plan because SQL rollback restores all mutable rows.
- Post-commit rollback must not restore the old blob byte-for-byte: it must
  switch back to the old root while embedding the *current expected* registry
  `_Version` low 32 bits, then let SQL generate another `_Version`. The old
  root remains in CAS.
- Recovery must record the prior presence/full image of
  `dbStruFinal<uuid>`. Whether rollback should delete a newly created marker is
  not yet proven.
- Derived `Params`/`Files` cache writes are not required for storage
  consistency, but omission must be verified with a platform session before
  claiming runtime parity.
- Online invalidation was subsequently proven by
  `../extension-live-validation.md`. It is asynchronous: an existing session
  retains its loaded generation, while a fresh session converges to the new
  generation after the server polling interval. No `DynamicallyUpdated` row,
  working-process restart, or native `ibcmd` invocation was required.
