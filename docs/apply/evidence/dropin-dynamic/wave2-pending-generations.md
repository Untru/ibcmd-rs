# Pending dynamic generations: second-wave evidence

Date: 2026-10-01. BASE: `13d9c6dd08806d38ffc4386883b53db6f2e985a4`.
Owned laboratory: `F:/ibcmd/lab/05/wave2/dynamic`; recovery/status and raw originals remain there.
This checkpoint concerns bounded storage publication on BSP 8.3.27.2214. Session,
warm/high-load, licensing `.ui`, other SI classes and 8.5 acceptance remain open.

## Native measurement

Two consecutive native `infobase config apply --force --dynamic=force` operations
followed delta imports. The first changed the existing root Title of CommonForm
`СвязанныеДокументы`; the second changed its existing Button Title, retaining the
first edit. Both stages also carried the two previously pending module bodies and
an unchanged inflated HELP `.1` companion. The source contains 12,198 files.

Raw snapshots: `snapshots/staged_first.json`, `native_first.json`,
`staged_second.json`, `native_second.json`. Native exports and comparisons:
`native-first`, `native-second`, `logs/first-source-compare.json`,
`logs/second-source-compare.json`. Every requested-source file equals its export;
the only normalization is `ConfigDumpInfo.xml`'s `configVersion` attribute values.
Both Title markers occur in the second exported form.

Each native force added six Config aliases: four body rows, `versions` and
`deleted`. Earlier aliases remained unchanged. Each added sixteen Params SI
aliases whose **raw bytes** equal the respective ordinary SI row. `siVersions`
is uncompressed BOM UTF-8 brace text, not raw deflate: its sixteen UUID tokens and
entry order changed. The exact first native raw sample is checked into
`tests/fixtures/dynamic-overlay/siVersions.native.txt`; a deflated copy is refused.

The initial marker generation is a measured exception: it has no Params SI aliases
and no Config deleted alias. Every later generation requires all sixteen aliases.
The exception is attached to the first **ordered** marker UUID; partial initial
collections and entirely missing later generations fail closed. A complete initial
collection is also unmeasured and refused.

Native register append deltas were six entries in generation one and twelve in
generation two. Only the three eligible exchange-plan nodes were affected, although
two additional stale/ineligible registrations existed for one module. The ordinary
register row count did not change and existing message numbers were NULL. Native
licensing `.ui` changes were observed separately; the implementation preserves its
preimage and does not reproduce or decode them.

The initial native delta-import stage used the preserved first-wave EXT binary,
SHA256 `36ed46bde89b4580d6fabcd2d2cf7d464188cdcbf266c74dd590056b6bb58323`.
Its provenance is staging evidence, not a claim that this final feature binary
created that initial stage. The own clone was restored from `bak/staged-first.bak`.

## Transaction and recovery boundaries

The pending Config and Params inventories are bounded before blob reads. Every
semantic blob is constrained by its initial size and digest; transaction preconditions
bind the complete service/alias image and history. Exact known deleted entries are
accepted, with zero flags; structural removals, duplicates and unknown names are refused.
The preconditions run before publication **and before no-op stage consumption**.
All staged ConfigSave rows require measured `Attributes=0`; the locked guard binds
their full Attributes/Creation/Modified headers as well as names, sizes and digests.
Changing flags or timestamps while keeping payload bytes unchanged is not accepted.
Config `*.new` phantoms and named unfinished-operation markers also refuse under locks.

Existing appended file keys use deterministic ordering, filtering already listed
names before row numbering, and the locked prior maximum. Duplicate inputs, negative
keys and integer overflow refuse without losing old entries. The dynamic-only exact
registration-ID filter excludes ineligible existing rows; default/exclusive callers
retain their previous scope. Node tables are bound by exact IDs and eligibility
classification, not just counts. Full touched-owner registration/list preimages
remain checked even for ineligible rows; an empty registration image receives a
locked zero-count assertion too.

Recovery saves replaced Params rows plus every exact missing registration-ID/key/name
triple from the CAS-bound preimage. The names can belong to an older generation.
After a confirmed COMMIT, the stopped-database manual recipe removes only those
new triples and new Config/Params generation aliases, then restores saved rows.
Transport errors around COMMIT retain artifact/token information and explicitly
require inspecting the outcome. This is not automatic resume or online undo.
The append recipe is created/truncated even when empty, so reusing an explicit
artifact directory cannot retain old append triples beside a new zero-addition manifest.

The first immutable review (`9e89f731`) found a recovery-only ABA gap: collection
judged siVersions A, generic artifact capture could reread transient B, and the
locked publication guard could subsequently admit restored A. Repair cycle one
now carries the bound original raw bytes and complete RowMeta from collection
into dynamic recovery, using an immutable object with private fields. Recovery
checks exact rewrite coverage/size/digest before replacing artifact files and
does not reread siVersions. The legacy/exclusive capture route is unchanged.
Fake-client cases preserve A's exact bytes and Attributes/Creation/Modified even
when current storage exposes B, no row, multiple parts or header-only drift;
invalid constructor metadata and missing/duplicate/stale rewrite coverage refuse
before SQL or artifact writes. This is not an executed recovery replay or a new
native matrix; the positive two-generation measurement still uses its named earlier build.

## Validation status

Native survey completed. Intermediate owned refusal probes cover stale stage,
service and alias bytes, Config `*.new`, unknown deleted entries and ordinary-row
no-op reverts with absent/empty deleted lists. Original storage and all eight stage
rows were restored exactly (`logs/negative-guards-intermediate.json`). Earlier
harness expectation corrections remain in the raw logs.

Two own actual consecutive publications completed using `bin/dynamic-wave2-final.exe`
and the corresponding `bin/ibcmd-rs-overlay-final.exe` write source. SQL execution
took 362 ms and 422 ms; total measured apply time was 1,169 ms and 1,140 ms. The
first stage came from the preserved original backup; the **second import used this
new feature CLI**, with eight staged rows. Both native exports of the own results
equal all 12,198 requested-source files, allowing only CDI `configVersion` values;
the first comparison uses the saved exact `snapshots/form-first.xml` alongside
the unchanged remaining source. Both edits survive the second export.

The committed evidence copies under `wave2/` include source comparisons, scoped
inventory comparisons, exact recovery append validations and the original TSVs.
The own committed additions are exactly the six and twelve pre-execution
recovery triples, including their registration IDs and keys. The two ineligible
registrations were untouched. Earlier Config/Params alias payloads were retained
raw; all 32 new SI aliases equal ordinary SI payloads raw. Independent second
import generation UUIDs and changed staged version tokens are explicitly mapped,
not hidden as arbitrary byte normalization. Native `siVersions` UUIDs/order and
the fresh mobile-ring UUID are nondeterministic and described separately.

Repeated drop-in force with no stage returned exit zero and published no third
generation. The actual generated append helper executed twice again, with zero
pending additions each time and no duplicate/changed keys. All six measured
table inventories and saved blobs remained identical (`repeat-force-noop.json`).

The first own attempt using `dynamic-wave2-eligible.exe` refused SQL 57318 because
the reused parity helper's all-registered-node count had been left zero. Its
runtime correctly reported uncertainty; an exact before/after inspection then
confirmed rollback and all eight stage rows retained. The count was corrected to
the original exclusive all-type/reference-pair query, separately from the three
eligible nodes. Failed script/recovery/proof remain preserved, distinct from final
success and from harness encoding/expectation corrections. Final guards also
refused a node classification change with unchanged node count.

The raw native/own payloads of three derived help/search indexes are retained in
`snapshots/help-index-raw`. Native first and second payloads are identical; own
payloads remain exactly the original preimage. Inflated bytes differ too, so
compression-only equality is **not** claimed. Strict UTF-16LE decoding failed for
`userPostings_ru.bin`; successful decoding of Docs and Vocabulary does not prove
their complete codec or semantics. No new codec or semantic equivalence is
claimed. This is the inherited uncovered
help-search index boundary already recorded by the exclusive apply, now also
reported by dynamic `not_written`. No full storage parity is claimed.

| Files row | Native stored bytes | Own stored bytes |
|---|---:|---:|
| userDocs_ru.bin | 47,456 | 47,528 |
| userPostings_ru.bin | 536,921 | 537,522 |
| userVocabulary_ru.bin | 105,654 | 105,710 |

Exact raw/inflated SHA256s and headers are in `wave2/help-index-raw-audit.json`.
Two known licensing `.ui` payload differences remain explicitly listed in the
scoped comparisons. Every other unexplained Files/Params difference refuses the
comparator. Active/warm/high-load, help search, licensing and other inventory
acceptance remain open. This binary's exclusive path retains its strict Params
alias guard: settle collected generations with native exclusive apply, not an
unproven direct-exclusive fold. A fresh exclusive-fold twin is not part of this proof.

After the actual DB matrix, report text for the help/search gap, recovery artifact
reuse hardening, immutable bound siVersions recovery and the stricter dynamic-only
stage header/flag preconditions changed.
The publication algorithm remained identical. The final source/binary has separate
focused stage-flag, full-header/no-op ordering and recovery-reuse regressions plus
matching quick gates; the two actual DB publications used the explicitly named earlier build.
The temporary driver was preserved in the lab and removed from the product tree.
Final binary hashes/gates are recorded in `wave2/checkpoint.json` and `STATUS.md`.
Final code/spec/evidence review passed on immutable `cce4ef91` after the recovery
preimage repair. The measured storage/client limits remain open; no completion
of issue #347 is claimed.
