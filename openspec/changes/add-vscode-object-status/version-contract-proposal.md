# Native version integration contract proposal (parent/#362 review required)

Status: proposed, not a finalized public RPC contract. The pure client adapter is
implemented/tested but deliberately not selected by the production preview yet.
The #362 owner owns native parsing/status semantics and reusable engine helper.
No #362 branch, CLI parser, native process adapter or duplicate version parser was changed.

## Existing engine assessment

- `stored_objects::Inventory::version_of` hashes row version entries into a 16-hex
  cache token. `ObjectStatus.version` uses that token. It is for freshness, not a
  native `configVersion`, not an object identity UUID, and not a file-content hash.
- Public `mssql_dump::stored_config_versions` returns **row-id -> native version**,
  but is Option-based and loses parse failure detail. It does not provide canonical
  native name/child ownership or the native counted-entry policy. Therefore it is
  insufficient for an honest standalone RPC adaptation.
- Existing private engine code already owns the necessary work:
  `config_dump_info::{current_config_versions,canonical_reference_names,
  child_reference_names,config_dump_top_name}` and `incremental::DumpInfo`.
  Reuse through #362's helper; do not expose a second parser in the extension.
- `ConfigDumpInfo` child entries can lack an independent version. The helper must
  supply parent/child identity semantics. The client must not compare a child ID
  or its file hash with its owner's version, nor split arbitrary FQN strings.

## Independent evidence model

Three domains remain separate:

| Value | Purpose | Permitted equality |
| --- | --- | --- |
| Native row `configVersion` | DB vs ConfigDumpInfo reference change | Native versions of matching identities, inside #362 helper only |
| Existing RPC `version` | Coherence/freshness of object evidence | Same object token across source/reference/native-evidence reads |
| Local/reference file bytes (or hashes) | Edits since synchronization | Same file path and byte/hash algorithm only |
| Baseline snapshot identity | Bind a report to the exact reference read | Same immutable baseline snapshot identity only |

`classifyVersioned(current, reference, evidence, fileChanged)` is an internal
adapter with evidence `{basis:"native-config-version",object,objectVersion,
baselineIdentity,conclusive,changed?}`. `objectVersion` is explicitly the existing RPC token.
The extension never sees raw native versions for comparison. The adapter additionally
requires an expected baseline identity and exact reference object identity; it refuses
evidence from another baseline even when all DB tokens are equal. Unknown/malformed
basis, inconclusive/missing change, wrong object, mismatched tokens, absent
reference or missing local evidence produce unknown for common objects.
Existing DB-only/file-only byte-existence outcomes remain distinct from native
added/deleted version records. A native deleted **part** does not imply that its
owning object is absent. Identity replacement/rename must be mapped by the server.

| Native DB changed | Local files changed | UI |
| --- | --- | --- |
| false | false | same |
| true | false | database |
| false | true | file |
| true | true | both |

The byte-difference union remains useful only for local/reference comparison:
any local/reference byte difference differs from DB bytes on at least one side.
It must never decide the native DB-changed flag.

## Proposed minimal additive RPC adaptation

Suggested method: retain `objects/status`, adding optional `versionBaselineDir`
(the already resolved read-only folder/Git snapshot). The server loads its
`ConfigDumpInfo.xml` with the shared helper. Legacy calls remain byte-only.
Each returned row may add `databaseVersionStatus` matching the internal evidence
shape above. Illustrative response for the observed Language mismatch:

```json
{
  "object": "Language.English",
  "status": "same",
  "version": "opaque-current-object-token",
  "changed": [], "baseOnly": [], "localOnly": [],
  "databaseVersionStatus": {
    "basis": "native-config-version",
    "object": "Language.English",
    "objectVersion": "opaque-current-object-token",
    "baselineIdentity": "opaque-reference-snapshot",
    "conclusive": true,
    "changed": true
  }
}
```

The server must compute/validate the native evidence against the same effective
CONFIG+CONFIGSAVE state and counted-entry policy as the shared native export-status
helper. If external mutation prevents a coherent view, fail or return inconclusive;
never attach a stale flag to a fresh token. The exact helper API, capability name,
RPC method/field name and snapshot strategy require parent/#362 agreement. Baseline
evidence MUST bind to the exact ConfigDumpInfo reference snapshot used for this
refresh/local comparison; a DB token alone cannot distinguish two references.
The final RPC adapter must return/verify that identity or an equivalent immutable
request-bound reference. This proposal does not choose its encoding or derive it
from a native version UUID.

Server maps native names/IDs to exact existing tree object names: native root
`Configuration.Configuration` -> tree `Configuration`; module/part records ->
verified owner; nested objects retain their verified identities. Any changed entry
for an owner makes that owner's DB-changed flag true. Empty output is unchanged
only after a conclusive complete comparison. Missing/malformed reference, unsupported
format, incomplete canonical name mapping, or unknown helper coverage must not
be silently treated as no changes. Unrelated unmapped records require an explicit
coverage policy agreed with #362; conservative fallback is inconclusive.

## Required regression and integration gates

Implemented pure tests use the measured mismatch from `native-read-20261022.md`:
Language byte same + native changed -> database; locally edited Configuration
byte-baseline same + native changed -> both. Tests also cover six states, missing,
malformed, mixed-basis, wrong-object, wrong-baseline and stale evidence. The adapter is dormant until RPC is agreed.

After helper delivery:

1. Rust contract test for actual tiny filedb/rows helper with changed versions and
   unchanged file bytes; assert exact mapped root/module/language owner coverage.
2. RPC legacy/backward compatibility plus opt-in version evidence; errors and
   partial mappings are explicit; no native process or filesystem output.
3. Client decode only capability-backed evidence; preserve abort/generation logic.
4. Native prepared filedb host test switches from documenting mismatch to asserting
   corrected module=database, Configuration=both, Language=database.
5. Native add/delete/rename/child-only changes, malformed/missing baseline, two
   refreshes with a DB mutation in between, folder and pinned Git reference.
6. True Configurator module edit + independent local edit acceptance, then parent
   review and PR gate. Whole-import evidence alone is not GUI acceptance.

## Implementation update after helper b46cd919

The helper is integrated as 9316ae33. The additive `versionBaselineDir` RPC and
`databaseVersionStatus` fields above are now implemented. Evidence additionally
carries `databaseSnapshot`, a SHA of the complete native inventory used only for
freshness; both native-mode requests must agree. Engine reads are bracketed with
that inventory and reference bytes. Production now selects the versioned adapter
by default. `statusComparison=bytes` remains an explicit labelled preview.

The completed owner helper a5fdd226 now replaces the temporary narrow mapper.
Canonical nested owners are retained in nativeOwners and rolled into explicit UI
root objects. Unknown roles stay inconclusive. SQL main-view adaptation uses the
existing CONFIGSAVE overlay for tree/bytes/versions with an independent cache;
read-only recording-client coverage is not live SQL acceptance.

Actual native selected-import all-six-state extension-host and folder/Git evidence
passes; see evidence/native-integration-20261022.md and the owner regression follow-up.
Configurator GUI and live SQL validation are not claimed by those measurements.
