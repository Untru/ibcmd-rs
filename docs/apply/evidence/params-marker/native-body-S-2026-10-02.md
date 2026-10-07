# Native N1 body S: marker preserved; console failure retained

This is one cold native storage control for #418, using 1C 8.3.27.2214.
It does not complete the marker matrix, prove OWN parity, or enable the
test-only candidate rule. No Rust CLI was executed in this control. The
historical harness/source checkpoint was `7260f896`; this publication changes
documentation only.

The original root tool session was 61597, outer PID 84684, controller PID 87976.
Its original Process exit and both original pipes were confirmed: **exit 1**.
Native partial import and exclusive `--force --dynamic=disable` apply both
returned 0, as did their two exact FIFO releases. All first 57 child receipts
returned 0. Step 058 wrote the UTF-8 observation file, then failed while printing
its BOM-bearing marker string through Python's cp1251 stdout. Its original
UnicodeEncodeError and exit 1 are preserved; there was no retry. The runtime's
final backup verification and outcome publication were skipped. A subsequent
offline readback verifies the stored data and backup; it is not a successful
rerun of step 058.

The already restored, manifest-owned DB70 was used without another restore,
registration, client, RAS or worker. Its whole SQL identity, exact ownership
row and unchanged idle predicate were checked before writes and after queues.
The complete initial six-storage-table/ten-auxiliary-table image matched the
natural N0 baseline. Each native write held one captured FIFO lease; final
state has no bound names, native potential or uncertain child. No cleanup,
signals, SQL KILL, drop, manual Params alteration or OWN apply occurred.

## Measured result

The real stage had five rows: `root`, `version`, `versions`, the CommonModule
descriptor `16b3681c-426d-4d6f-9ffe-588a23974222`, and its `.0` body. The
descriptor's compressed representation changed while its complete decoded
bytes were exactly equal; the body genuinely changed. Native apply consumed
the stage. This extends the historical F4 counterexample with complete physical
header evidence, rather than treating compressed SHA or descriptor presence
as a semantic change.

Params `DynamicallyUpdated` remained exactly **156 bytes**, with the same
creation/modified dates, attributes, DataSize, PartNo and raw payload SHA256
`a6a4becf0894197ff52642b07a26dafd9c9b5298e084a615a031a54b4906a1e6`.
Its UTF-8 BOM and four history UUIDs are retained in the raw observation.
All ordinary SI and `siVersions` rows remained exact.

Every other delta was retained without normalization:

| Storage | Delta |
|---|---|
| Config | Five stage-related rows changed; 9,836 rows exact |
| ConfigSave | Five staged rows removed |
| Params | Three opaque `.ui` rows changed; 36 rows exact, including the marker and SI |
| Files | `MobileVersions.dat` changed; 352 rows exact |
| ConfigCAS | All 12,797 rows exact |
| ConfigCASSave | Empty, unchanged |
| Ten auxiliary tables | Exact column inventories and row multisets |

The first `.ui` change is Modified-only; the other two change Modified and
payload SHA. These opaque effects are listed as measured differences, not
ignored, decoded, or called full Params parity. Raw headers and digests for
all differences remain in the nine full snapshots and independent physical
report. The precise collection rule, genuine Comment/Synonym changes, repeat,
effective pending aliases and 20/21-row boundary still need their controls.

## Preserved twin and evidence

The staged COPY_ONLY/CHECKSUM backup has **254,435,328 bytes**, SHA256
`84A972325B50FF6E4A4D68A82CDAB16C898CCA8973CEE1EE3EF1B847A0A14492`.
Its complete HEADERONLY binds DB70 name/GUID, family/fork and the measured
second-precision creation time to the captured SQL identity. VERIFYONLY
returned 0; full six-plus-ten snapshots before and after preservation were
exact. The backup bytes/header were independently read again offline after
the console failure. It is retained for a later freshly restored OWN twin;
no such twin has been applied yet.

The immutable terminal checkpoint is
`E5E357EA83FD31647D011FF69D4FF649D539F846FA36D3434DC3A7BDE7E4F518`,
369 members, preserving all consumed 169 members. Offline analysis recomputed
all nine six-plus-ten snapshots, 54 complete storage packs, each row range and
SHA, complete headers, contiguous multipart totals and six exact baseline,
queue and preservation comparisons. Both root and the independent peer
accepted the scoped scientific result while retaining the original exit 1.

[Publication checkpoint](native-body-S/checkpoint.json) binds exact copied
evidence bytes, original locations and digests. The package includes original
terminal/final state, failed 058 receipt, observation, full staged header,
native/lease receipts, independent analyses and review certificates. Large
packs and backup remain in the F-drive laboratory at the paths and SHA256s
listed in the preserved terminal checkpoint. No future runtime or producer
is certified by this documentation commit.
