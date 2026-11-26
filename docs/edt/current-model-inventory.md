# Current EDT reader evidence

Issue #355 now targets current EDT validation while retaining the explicit
2025.2.3 profiles and their existing schema evidence. A tested executable build
and a codec model profile are separate coordinates. Neither running the legacy
writer in a newer executable nor matching class names proves current schema
support. Do not relabel the legacy profiles.

`scripts/edt-lab/model_inventory.py` inspects installed JARs without executing
EDT/JVM, extracting archives, changing the installation, or creating workspaces.
It compares every bundle and each model/importer/exporter class in the bundled
legacy inventory. Missing or ambiguous bundles remain explicit. It records JAR
hashes before and after inspection, rejects changing inputs, and records hashes
of every `.ecore`, `.xcore`, and `.genmodel` resource in the inspected bundles.
The output is created exclusively; earlier evidence is never replaced. Output
must be below an existing external `/Volumes` lab with no symlink ancestors.
The mounted filesystem must differ from the root filesystem; read-only `diskutil`
metadata must identify the volume and every APFS physical store as noninternal,
nonsystem storage on USB, Thunderbolt or FireWire. Missing or unverifiable
metadata fails closed. This diagnostic supports those explicitly verified Mac
storage transports; a pathname prefix alone is insufficient. Output creation is
exclusive and bound to an open directory on the verified device.

Model resources are streamed with a 256 MiB per-entry diagnostic cap and a
shared **256 MiB run-wide decompression budget**, including manifest main
sections across all inspected bundles. Reports retain the budget and actual
consumption. Exhaustion writes an explicit `LIMIT_EXCEEDED` failure report and
exits nonzero; it never emits truncated success evidence. Manifest main sections
are bounded to 64 KiB and recognize CR, LF and CRLF section boundaries, so signed
per-entry digest tails are not read. The archive is inspected and hashed through the same open
file descriptor, with final pathname and descriptor identity checks.

Use exact paths under the external laboratory for the output and any shell
logs. Disable Python bytecode caching so imports do not create local caches:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/edt-lab/model_inventory.py \
  --plugins "$EDT_PLUGINS" \
  --baseline crates/ibcmd-schema/data/edt-2025.2.3-model-inventory.json \
  --installed-release 2026.2.1+3 \
  --lab-root "$LAB" \
  --output "$LAB/logs/edt-2026.2.1-model-inventory.json"
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s scripts/edt-lab -p test_model_inventory.py -v
```

The release argument is a declared coordinate, not a verified executable
version or a claim to be the latest official release. Bind actual CLI version
and executable identity separately using #358's native capture.

On the Mac's installed **2026.2.1+3**, the initial read-only comparison inspected
all **76** legacy bundles and hashed **64** model resources. Two legacy model
class names are absent: `ObjectOnSupport` and `impl.ObjectOnSupportImpl` in
`com._1c.g5.v8.dt.compare.model`. No other legacy model/importer/exporter class
names were absent. New classes, unrelated/new bundles, method bodies, feature
semantics, serialization defaults and ordering are outside this comparison.
Resource hashes permit later exact comparisons; they do not establish that
legacy and current resources match.

This diagnostic is not a reader/form fix or full configuration acceptance.
The historical 95 form differences still require a current-master paired
BSP/UH census. Authentic EDT import/export and native comparisons remain
separate gates. Current-build empty-project smoke evidence cannot establish
whole BSP/UH support, arbitrary-configuration support, or legacy exact-version
acceptance.

## Witnessed current form framing fix

The shared genuine native `DcsFilterEvidence` import into installed EDT
`2026.2.1.3rc0` produced two LF-only managed forms (45,086 and 11,724 bytes).
The pinned-master form reader rejected both with `must be CRLF, found Lf`.
The corresponding native XML forms (19,444 and 8,283 bytes) already passed
byte-exact and typed same-dialect regeneration.

The reader now captures a source-only LF framing facet from the XML declaration
separator. The EDT writer uses that facet for structural and in-text framing;
new bodies and legacy CRLF sources retain the previous CRLF behavior. LF sources
use the existing verbatim-text parse policy so a typed edit containing explicit
CRLF survives rereading. Native LF XML, BOM-bearing EDT and unknown form nodes
remain rejected. The facet is excluded from semantic serialization; this is a
witnessed framing capability, not a new current EDT schema/profile declaration.

Two minimized in-memory regression tests cover both XML dialects, edited text
(including explicit CRLF), serde exclusion, legacy output and negative cases.
After the fix, all four genuine current/native form files read and regenerate
byte for byte, and each same-dialect typed fingerprint is unchanged. All inputs
remain unchanged. This was an offline probe using the captured current EDT pair,
not a new installed-EDT run or full configuration acceptance.

Standalone cross-dialect `CorpusList` typed fingerprints agree. `FilterProbe`
still differs in `conditional_appearance` and `data_attributes`; a full current
metadata-context/native comparison must classify those differences. Neither
these two forms nor this framing fix settles the historical BSP/UH census.
The separate configuration-interface sidecar incompatibility is owned by #357.
