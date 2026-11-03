# Shared public SQL probe fixture readiness

Status: source and sanitized operator-receipt inspection. The reviewer executed
no SQL/native/VM operation. A native SQL source baseline is now measured below;
this packet is still not a capture plan or an accepted reader/writer profile. The #380 read-only RPC packet
and #382 profile capture may share one source only after the sole operator proves
and freezes the exact new disposable SQL fixture under an assigned lease.

## Known source inputs

| Input | Source-inspected evidence | Permitted conclusion |
| --- | --- | --- |
| Bootstrap manifest | `tests/fixtures/bootstrap/manifest.json`, SHA-256 `07ca63b632e6b9915fcc809400c49ca83601a9678c49bcadf4aba44abe661f3a` | Hand-authored clean-room 8.3.27.1989/XCF2.20 common-module seed design. No native SQL creation/apply proof. |
| Primary snapshot batch | `src/server/sql_import/snapshot.sql`, SHA-256 `ae96e854c44ee78ff796586de10bdeafb24698be302bcad8a76a7f70647c6ac5` | Fixed read-only acquisition source, no accepted measured SQL profile. |
| Operator capture batch | `scripts/sql-lab/snapshot-profile/capture.sql`, SHA-256 `57d42f13bf4996c4723a48d2876a0b4610dff5b5698871428a5be84328897772` | Reviewed source-only metadata/header/part capture package, not a live receipt. |
| Plan validator | `scripts/sql-lab/snapshot-profile/validate.py`, SHA-256 `0fbadd8b0a6eecb482078fb5d6a833f00dd869d05e34aabaac81f48748def3b3` | Strict offline structure validation; accepted_profile stays false. |
| Public chart candidate | Public source/evidence inspected at `ut-export-diff` commit `761e7ca4c79b779e9ccb9e9001648b5ece50c1f5`; exact bindings below | Source-owned seed with candidate-compiled CF and recorded FILE observations only. Not SQL create/apply/export evidence. |
| Public-chart source controls | Retained public control receipts explicitly state `windows_or_existing_baseline_executed=false` | Pure immutable-capture/growth control evidence only; not the chart native baseline. |

The public 8.5 tiny file-infobase fixture does not prove 8.3 SQL compatibility.
Private application corpora and existing user databases are excluded. No target
is guessed from old editor connection labels or prior application measurements.

## Public chart seed bindings (source-inspected)

The fixture owner is #381; the source worktree is `repos/ut-export-diff`. Input is
ONLY `tests/fixtures/ut-chart-policy/wrapper-prototype/tree/`: all five file
names, lengths and SHA-256 values match its manifest on read-only inspection.
The manifest SHA-256 is
`aff4c4f4f7d84cb08866bdb0e9190b5bac8a4b26c6d0d477c4f2a676ac7c27c4`.
Its historical `UNMEASURED_NOT_NATIVE_ACCEPTED` status remains unchanged.

Later public evidence is
`tests/fixtures/ut-chart-policy/explicit-english-zero-series/evidence.json`,
SHA-256 `f58e71b1c68a95d3cc3d0ecff8a5862ac28b1f7d7df2fb015f97b1230983ca9c`:

- Candidate-compiled CF: 7505 bytes, SHA-256
  `b2fa2d388face5e7281711d3714815cc304e12bd220f7495be15abfa0a613b8b`.
- Recorded native build: 8.3.27.2342; executable SHA-256
  `38f37715535cea530e58d5ba2e7e072c184b61b12f3e903ac290b299cf39fd38`.
- Referenced retained native receipt SHA-256:
  `f65179de7a49cdb7acad0225a1a3853bb0215b70a3404221e6db26f1e7b3c493`.

The CF and candidate-only receipts were independently hashed at their retained
external lab paths (relative to `/Volumes/Work/Dev/ibcmd-rs-lab`):

| Artifact | Retained path | SHA-256 |
| --- | --- | --- |
| CF, 7505 bytes | `fixtures/ut-chart-wrapper-basefree-internalinfo-admission-a1d7357862ff4658a6d4a00bdfecf08f/prototype.cf` | `b2fa2d388face5e7281711d3714815cc304e12bd220f7495be15abfa0a613b8b` |
| Candidate bootstrap receipt | Same directory, `receipt.json` | `72784cf79d837e85c5508ae31e492d53abf18cb1e69858d6d6cb5901a8f5e7aa` |
| Candidate readback receipt | `fixtures/ut-chart-wrapper-readback-48a5f728eb2b4c15a39de6f91073647c/receipt.json` | `dd2ce56d4678a5cd511c4672e7f29112163d22c38fe8ac56c114a49034dd16e9` |
| Public bundle, 13564 bytes | `tmp/ut-chart-wrapper-candidate-public-003.tar.gz` | `34739b89df57ac4e7af6a2fc8879084d358cd59abe606c9ac2a66f4792409b9c` |

Both inspected candidate receipts explicitly record `native_runs=0` and
`input_is_not_native_oracle=true`. Their successful bootstrap/readback is not a
native receipt. The sanitized retained native FILE receipts supplied by #381 were subsequently
read-only inspected and independently rehashed:

- `lenovo-lab-inspection/artifacts/chart-native-baseline-v2/baseline-aggregate.sanitized.json`,
  SHA-256 `8ca5f63ce04282a61785cee4732106a907cbd8a92b714a0b13ae19bcf2697dbb`.
- Same directory, `native-baseline-outcome.sanitized.json`, SHA-256
  `acdf051c92e038dad5821e83897f6dd0c9f8d5500c1e181a68ccd9e247e1ad1a`.

They record one invocation with four phases (`native-version`,
`native-create-empty`, `native-load-public-cf`, `native-export-baseline`), all
exit 0 and stderr 0, with the same public CF before/after. The aggregate explicitly
records `SQLUsed=false` and no native output tree/private XML transfer.
The original receipt hash is the referenced `f65179de...` above; that original
VM receipt and executable were not fetched or re-executed here. The sanitized
outcome is `FINAL_AUDIT_INCOMPLETE` with `inventory_entry_budget_exceeded`.
These are historical FILE admission/name observations, not complete parity,
SQL creation/schema/identity evidence, a fresh run or an operator lease. The
evidence explicitly records `native_full_acceptance=false` and incomplete final
accounting. The CF is candidate-compiled public source, not native expected
output. A FILE import/export observation is not a native SQL apply baseline.
The shorthand combining CF prefix and 7505-byte length is not a full hash.

## Actual public native SQL baseline (receipt-verified)

The sole operator completed create/load/apply/save-CF/CDI/XML on native
8.3.27.2342. Independent read-only rehash/inspection confirmed:

- `lenovo-lab-inspection/artifacts/public-sql-native2342-baseline/manifest.sanitized.json`,
  SHA-256 `aa22ac9eafc3bb9f655c3e41a9e9028e58ba8fa79a633090a20b9b4c08d4cac4`.
- `lenovo-lab-inspection/artifacts/export-only009-54c8096f-871d-491e-b7c0-0e6dd9a037ab/release-aggregate.sanitized.json`,
  SHA-256 `1077c6c57b8de08774ef11894e67e34118de5b5e37da4d053098ea97a89437d1`.

The composed chain preserves the initial canceled apply and failed CDI-path
receipts; it does not rewrite those runs as success. A separately approved
single exact-warning response completed apply/save, followed by export-only CDI
and XML. No create/load/apply/save was blindly replayed after successful phases.

Actual sanitized target bindings: server `WIN-LHDQGRLB1L1`, database
`IBCMDLAB_PUBLIC_SQL_151c08962516426e96ccf4e48d240ff9`, database ID 12,
GUID `9000716B-B12F-45BE-BB5B-27FB88534F95`, family/recovery fork
`DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC`, SQL ProductVersion 16.0.1000.6,
collation Cyrillic_General_CI_AS. Identity is unchanged and the recorded final
observation has no other target user sessions and FULL RELEASE. This historical
quiescence observation is not a future lease or freeze interval.

VM-local outputs: saved CF 77760 bytes SHA-256
`4baf69f561e84e22b8986e6ab95d74ee83bf80373f971aa1454eb1ea9dbe33e7`;
6 XML files/25135 bytes, inventory SHA-256
`4bcc455f8d1a4a61b20546951bcd4b403c3d58b8ce0fe8e0589440f0a164e9cc`;
CDI 1101 bytes SHA-256
`d26f3f250f563454701dabc537ac116f6240bef21485f5326350e9ace182ca46`,
identical to the CDI within full XML export. Raw files remain in the VM. The
original public CF, saved CF and prior receipts remain unchanged.

This completes native source preparation, not #382 partial module/form staging
acceptance. Strict SQL snapshot/schema/header/parts measurement, actual reader
comparison, network TLS and guarded publication remain separate. The fixture
contains a module/template; no document/form roundtrip is implied.

## Phase boundary: source preparation versus capture

The missing outputs below are requirements before candidate/capture probes, not
prerequisites for creating their new disposable source. A new database GUID,
measured schema/header/part profile and native SQL baseline cannot exist before
creation. Their absence does not block a separately reviewed source-preparation
plan using an already authorized identity and existing security configuration.

Pre-create review binds the exact public CF and native executable/help, actual
existing SQL server/principal/auth mechanism and permissions, a fresh owned
run/data/temp/output namespace, and absence of the unique
`IBCMDLAB_PUBLIC_SQL_<runid>` target. The sole operator must have released the
preceding phase and hold the new lease. Exact native create/load/apply/export
arguments come from actual inspected help/configuration, never guessed syntax.
No originals, security changes, force/drop, or retry against an existing name.

After creation/native load/apply/export, the operator measures the generated
GUID/family/fork, SQL version/schema/header/parts, baseline fingerprints and idle
freeze before candidate #382/#380 runs. Those outputs populate the existing
strict snapshot-capture plan; that validator is not weakened or used as the
pre-create source-preparation validator. Measurement alone does not certify a
writer profile or transfer capture authentication to Rust RPC.

## Capture runtime control (measured separately)

The PS5.1 compatibility helpers have actual operator Windows conversion-only
10/10 PASS, no SQL connection construction/Open and no capture invocation.
Independently rehashed receipt: `lenovo-lab-inspection/artifacts/snapshot-runtime-control-f29671fb-3358-4f86-82b1-ece0df199f8b/release.sanitized.json`,
SHA-256 `e8279d993efd9b6053aaea1ab6927b7a776f7a37c9f2b608c3d74c06ad6787ca`.
This binds wrapper `0d133311...` and conversion control `66bd3299...` on
PowerShell 5.1.20348.558 / .NET 4.0.30319.42000; it does not prove provider/capture.
A future changed measurement-plan/schema path needs its own reviewed controls.

## Capture/candidate bindings still to measure (STOP before those probes)

- Fresh operator custody/freeze/current lease for the now-known native SQL
  source. Historical FULL RELEASE is not an active measurement window.
- Actual one-session storage schema/header/part profile and full physical
  readset/fingerprint, bound to the measured identity above. Native source
  preparation is complete; it must not be repeated to satisfy this field.
- Exact current tested candidate binary/embedded revision and matching RPC packet
  provenance; source-only observer tests are not an accepted SQL profile.
- Source freeze/exclusivity receipt and window, precise baseline hashes, and
  current sole-operator lease after the preceding operator phase releases.
- Proof of the supplied dedicated non-pooled capture connection/provider
  capabilities. This does not bind #380 Rust `serve` settings/authentication.
- Independently verified current #380 product binary/settings/auth mechanism
  binding (reference only, no values), and its actual Windows seven-RPC probe
  passing with zero skipped cases; neither is established by this packet.

The public disposable target identity is recorded above; credentials are not. Authentication
reference for the #359 capture package is a mechanism only: the sole operator
supplies an already-open,
dedicated NON-POOLED `System.Data.Common.DbConnection` using their existing
integrated/token/secure authentication mechanism. No credential or connection
string is read, copied into a plan, printed or supplied as a CLI argument. The
package changes no login, privileges, network, encryption or trust settings.
This connection is not an inferred Rust `serve` settings/auth binding for #380.
Sharing a public seed does not share target identity, authentication authority,
product settings or a runtime acceptance result between the two probes.

## Exact first offline action for the capture phase

After the separately reviewed source-preparation phase produces the generated
bindings and the operator verifies the remaining capture bindings, populate
a fresh external private `plan.json` using the strict source-only README schema,
verify the reviewed package hashes, then run:

```text
python validate.py --plan EXTERNAL_PRIVATE_RUN/plan.json
```

Do not manufacture placeholders to make validation pass. Without those bindings,
stop before capture/candidate dispatch. This does not forbid the separately
reviewed native source-preparation phase described above. Offline plan validation does not grant a VM/SQL/native
lease or authorize database creation. After the sole operator separately obtains
the lease and disposable-target authority, the package permits one parameterized
ExecuteReader against its supplied existing dedicated connection. No USE,
reconnect, discovery fallback, or automatic provisioning is part of capture.
Actual resultset ordinals, provider cancellation, lock/coherence and source/native
preservation remain unmeasured acceptance gates.
