# Exact 8.5.1.1150 initial CommonModule dynamic publication

This checkpoint admits one measured existing CommonModule `.0` change through
the drop-in `infobase config apply --dynamic=force`. It does not complete #346.
Native force is supported by 8.5.1.1150; the previous blanket refusal was a
product evidence boundary, not the platform's syntax limitation.

## Admission and independent axes

The profile adds only `mssql.config.apply.dynamic`. The planner still requires
exactly five staged rows: existing CommonModule descriptor/body and
root/version/versions; unchanged descriptor/version, genuinely changed bounded
UTF-8 BOM module text, exact two-element V8 container and measured fifteen-byte
info record. Complete raw DEFLATE StreamEnd and all-input consumption are
required. The opaque root has tag2, the same canonical UUID, exactly128 payload
bytes and identical first112 bytes; only its final16 may change. Root parsing
is bounded before UTF-8 and accepts no outer padding. All semantic reads are
digest-bound to the physical image asserted inside the publication transaction.

Initial Config/Params markers, history, aliases, deleted lists and SI collection
are refused. The touched owner has exactly three eligible NULL-message change
registrations and existing `.0` file lists; node membership/full registration
preimages, NULL messages and counts are locked before publication. No new
registrations or appended filenames are admitted. Stage/header drift, unknown
owners, forms, extension changes and subsequent generations remain fail-closed.
Existing 8.3 behavior and released 0.4 exclusive-apply policy are preserved.
Main activation, extension writes, ONLINE/LIVE/WORKER capabilities stay closed
for 8.5. This is not a generic worker ownership or session-refresh protocol.

SQL-only apply verifies the live schema fingerprint and IBVersion against the
declared profile; it cannot independently identify a server executable build.
An explicitly named exact build without its own native storage profile is
refused before SQL rather than falling back to a release profile. The final CLI
refuses known XML build 8.5.1.1529 at that boundary and unknown build 8.5.1.1151
at argument parsing; declared release aliases retain their existing behavior.
The build is the caller's claim. This native/runtime acceptance independently
used installed 8.5.1.1150 CLI, RAC, ragent and thin client in a newly created
private cluster on6540/6541/6545. Registry history contained manifest-owned p85
infobases only. Disconnected registrations are included in snapshots before
signals; retained PID/start/exe/command identity union survives anchor exit.
This fresh-cluster proof does not establish loaded-unregistered inventory for
arbitrary clusters. Product worker capability is not enabled by the kit.

## Native twin and actual candidate provenance

Lab root: `F:/ibcmd/lab/05/wave3/platform85`. Owned SQL twins:
`ibcmd_rs_05_p85_w3_own_20261001` and
`ibcmd_rs_05_p85_w3_native_b_20261001`. Both were cloned from the same native
five-row stage. The baseline was already support-disabled by native exclusive
apply; the initial mixed support-disable/module measurement is retained but
is not used as a code-only cohort proof. No reference infobase was written.

Native partial import used `config import files --partial` on
`CommonModules/ОбсужденияСлужебныйКлиентСервер/Ext/Module.bsl`. Native force
and own force both published canonical generation
`79f150a7-6688-4795-8fd6-e3e1177b70f3` from staged versions, without terminating
old sessions. Actual OWN write used source HEAD
`6144fc4ab2c4473887123b33bbc336245fa05d44` plus the retained private single-line
profile toggle; binary SHA256
`521373855608b9bb12f59307042f8e6c43408837ea81fcef77cbbc089ec98b32`.
The fresh four-thread iter build and exact patch are recorded in
`logs/candidate-{provenance.json,private-profile.patch,iter-build.log}`.
The committed profile was restored unsupported immediately after building;
this checkpoint subsequently publishes the same admission with docs/tests.
The final checkpoint binary is verified separately; native writes are not
claimed to have been rerun after these docs/test/profile-evidence additions.

OWN apply rc0, SQL274ms, total1245ms; schema fingerprint
`49ab07a8ddb8c87ae1bdc47b1b5342dc2341dbf99cc777ede57ad8ec539bc0a3`.
Report `logs/own-b-candidate-report.json` names three aliases, root/version
replacements and the recovery artifact/token. Recovery bytes are retained under
the lab temp directory; they are manual recovery evidence, not generic undo.

## Full storage examination and source/runtime proof

Every raw row includes name/part, dates, Attributes, DataSize, actual length and
SHA256; payload bytes are retained in per-table packs. Six storage tables plus
eight auxiliary tables were compared, not just the changed module:
`snapshots/{staged_b_own,staged_b_native,before_own_b_write_warm,after_force_b_native,after_own_b_candidate}`.
The staged twins are exact. OWN startup introduced no storage/header delta.
Native and OWN final names/parts match; ConfigSave is empty in both. All Config
payloads, root/version/alias headers, CAS/CASSave rows and all eight auxiliary
tables are exact, except the following explicitly retained differences:

- Config/Params `DynamicallyUpdated` Creation/Modified are each operation's time;
  bytes, Attributes, size and length are exact.
- `Files.MobileVersions.dat` has independently fresh canonical head UUIDs and
  timestamps. Both1000-entry lists have exactly the same tail, equal to the
  preimage shifted by one with its oldest entry dropped. No other Files row,
  including help/search cache rows of every present language, differs.
- Three Params `.ui` rows differ after native runtime activity. OWN retains each
  full warm preimage exactly; two native hashes and three Modified values differ.
  Raw bytes/headers/SHA remain preserved. These are licensing records, never
  decoded or written by the implementation. Full Params byte/header equivalence
  is not claimed.

`logs/own-b-raw-comparison.json` records every delta;
`logs/own-b-storage-assertions.json` rejects any unclassified difference. This is
not blanket normalization or full raw storage equality. Both final native XML
exports compare exactly:12336 identical files,0 differences and0 missing paths,
including ConfigDumpInfo.xml without configVersion normalization. Raw
`logs/own-b-native-source-diff.json` and both source trees are retained.

Old SID1/PID93244 remained `P85_NATIVE_A/P85_NATIVE_A` in550 observations;
new SID2/PID70952 read `P85_NATIVE_B/P85_NATIVE_B` in371 observations. Their
polling spans overlap375071ms. Open records and every subsequent observation
are validated in `logs/own-b-session-assertions.json`; raw journals
`obs/own-b-{old,new}.log` are retained. This proves separate old/new cohorts,
not refresh of the same session. Initial assertion treated the valid `open`
record as a poll and failed; that harness failure is retained separately from
the corrected exact open-plus-poll assertion. Runtime observations have no
error records. No marker evidence is inferred from a launch or dialog log.

Before cleanup a complete registry/process snapshot was saved. Exact owned
observers were stopped, own registration removed, then the retained-identity
private stop verified no owned processes/listeners and released the worker
FIFO. Other clusters/processes were untouched. Run-specific copies retain the
first kit's foreign-after/identity evidence; fixed-name kit snapshots from
successive starts are not claimed to be one immutable lifecycle baseline.
Owned DBs remain unregistered for subsequent distinct controls/default60 cleanup.

## Remaining acceptance

Managed-form and extension native/own controls, repeated 8.5 overlays/SI service
collection, same-session refresh, generalized worker ownership and warm LIVE
activation remain open. The new profile regression checks exact admission,
unknown builds, unchanged8.3 policy and still-closed main/extension activation;
the initial-cohort/stream-completeness guards have independent focused tests.
Matching final CLI provenance and mandatory quick logs are recorded in the
checkpoint manifest. Independent immutable review is required before integration.

Final mandatory quick checks all pass: fmt, policy guard, clippy and root library
(3688 passed, zero failed, ten ignored). Focused profile tests pass twelve cases;
the exact-build admission regression also passes. The final four-thread iter
CLI SHA256 is
`3e223c871f5d13d488081b8e7726b777fcfe3e40f0ee6688c9e73da76d03687f`.
`initial-common-module/final-provenance.json` binds source hashes, binary, tests
and refusal logs. The actual native/OWN write remains the separately recorded
experimental candidate above; this final binary was not used to repeat it.
