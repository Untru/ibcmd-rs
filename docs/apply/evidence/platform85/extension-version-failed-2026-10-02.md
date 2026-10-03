# Exact 8.5 extension Version control: import refused, cleanup completed

The single native ServiceDesk Version control on exact platform 8.5.1.1150
failed during import. Native activation and a new observer were not launched.
The result does not admit product extension writes or demonstrate an installed
Version transition. Issue #346 remains incomplete; no CFE was built.

## Fixture and actual execution

The owned database was `ibcmd_rs_05_p85_w3_ext_version_native_20261001`, restored
and settled in the separate previously recorded cold control. Its actual native
extension export contains 633 files and Version 1.8.3.0. The new fixture changes
only the Configuration.xml Version scalar to 1.8.3.1; all other 632 file bytes
are unchanged. These are baseline and intended fixture versions, not proof of
installation. The old thin session SID1 actually reports
SessionAppliedVersion/DBVersion `1.8.3.0/1.8.3.0`. This is a server Version
observation, not a client-code marker or a same-session refresh result.

Execution used the independently reviewed frozen V4 controller, SHA256
`d5cd30786ea08b89816f712b9ac9086441372b80544f4d32542051d448b7a368`,
with 1,318 dependency/source/fixture pins under manifest SHA256
`d4e662c47961b66c268bd3a2e36fee1b28ba6b2de0cb4da6d0f02e18aab30b5e`.
Post-worker and post-heavy closure checks precede the first mutation. Actual
statement-order tests cover changed hashes after either queued acquisition.
The private cluster and old observer opened successfully. The runtime Rust
source reference was `288193085e473c852e64cc5a86ca825bca0113b0`; no Rust CLI was
executed in this native-only control, and this later evidence commit does not
claim another native run.

The exact native command was `infobase config import files --extension=ServiceDesk
--partial Configuration.xml`, using the unchanged complete B fixture as base.
It returned **-1**, reporting unresolved predefined references
`Enum.СтатусыИзвлеченияТекстаФайлов.EmptyRef` and
`Catalog.Пользователи.EmptyRef` (three occurrences). The four references occur
in unchanged native-exported `Catalogs/сд_ПрисоединенныеФайлы.xml`, not in the
modified Version scalar. The original command arguments and error are copied
verbatim in [native-import-command.json](extension-version-failed/native-import-command.json)
and [native-import-result.json](extension-version-failed/native-import-result.json).
No syntax fallback, import retry, fixture normalization or new metadata was used.
The intended apply and new-session phases were skipped.

## Retained session and exact cleanup

The old observer process stopped, but its own disconnected session remained
`hibernate=yes`, with zero connection/process UUIDs. The original controller
refused unregister/stop and retained the worker. Its
[failed final state](extension-version-failed/original-failed-final-state.json)
is preserved without replacement by the later successful cleanup state.

A separate frozen cleanup, SHA256
`ffce45315c6146c3c8e1856dfe91b7d72a0431a324334b2ef6a610ed39b39eb1`,
was independently reviewed against all 1,337 base and supplemental pins.
Its actual functions/sequence/finally tests pass 27 pure scenarios. The cleanup
first captured six full configuration-storage tables and eight auxiliary
tables **before** terminating only the saved own session UUID
`a7b6996c-cf01-4cba-9016-bb32df51a3ae`, in sole owned IB
`39794873-ac47-4cc7-86da-2dfb0b34a172`. Exact saved bindings, dead producer,
owned anchors, worker lease, empty connection/process fields, SQL idle/zero
transactions and child-completion authority were checked again before action.

Termination, standard unregister, guarded cluster stop without purge and exact
worker release all returned zero. The [cleanup final state](extension-version-failed/cleanup-final-state.json)
and [postcheck](extension-version-failed/cleanup-postcheck.json) confirm no active
private cluster state, saved live process identities or own worker lease.
The database was retained; this control did not drop it. No global session
operation, SQL KILL, product write retry or purge occurred.

## Full postfailure storage comparison and evidence

The warm pre-import and postfailure snapshots are exactly equal for every
captured configuration-storage row, including all dates, Attributes, DataSize,
lengths and raw bytes. The offline comparator verifies whole-pack and row-range
SHA256, contiguous multipart groups and group DataSize, plus all eight auxiliary
row multisets. Counts are Config 9,948; ConfigCAS 19,362; ConfigCASSave 0;
ConfigSave 0; Files 373; Params 38. There are no added, removed or changed rows,
and no content/header normalization. Pack offsets identify capture locations
only. This covers the captured configuration storage, not every business table.
See [postfailure-physical-delta.json](extension-version-failed/postfailure-physical-delta.json).

The immutable [result checkpoint](extension-version-failed/result-checkpoint.json),
SHA256 `deadb195c84bfe3eaf0061072b2339e49ada75abd5a9fec8dc4d05f0746171ca`,
indexes 2,047 original files, including full raw packs, journals, command results,
fixtures, stopped state and accepted source/dependency closures. Originals remain
under `F:/ibcmd/lab/05/wave3/platform85`; the repository keeps selected byte-exact
copies with [copy hashes and original paths](extension-version-failed/copies.json).
Copied scripts are immutable evidence, not a new runnable product or lab entry
point; their original sibling names and frozen lab paths remain authoritative.

Root independently recomputed the six packs/eight auxiliary multisets and checked
the complete checkpoint; records remain in
`F:/ibcmd/lab/05/wave3/coordinator/review-extension-v4-postfailed.json` and
`review-extension-cleanup-v1.json`. A future Version-only control must resolve the
existing borrowed/reference boundary and obtain fresh preparation acceptance.
This failed run provides no execution authorization or extension capability.
