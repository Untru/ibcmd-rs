# Optional context/query observer: local validation

The optional mode binds the compressed Form body and both frozen XML byte streams
by SHA256. A reviewed caller supplies VM-local exact attribute-name/MainTable
context. Rust checks custody, unique raw-attribute correspondence and paired
MainTable equality; it does not independently attest how the caller extracted
that context from XML. Context strings are excluded from report and Debug output.

The final-SELECT parser retains repeated plain paths only in diagnostic mode.
The ordinary parser uses the same implementation with projection disabled; its
paths/aliases/stars/source results and consumers remain unchanged. A public
equivalence control compares both modes after removing diagnostic-only fields.
No production field-resolution or export policy is changed.

## Passed local public gates

Rust 1.95.0 on macOS ARM64, jobs=1, external target/temp, IBCMD_TEST_KEEP_FIXTURES=1:

| Gate                                                                                | Result               |
| ----------------------------------------------------------------------------------- | -------------------- |
| `cargo test --locked --lib field_map_observer --no-default-features`                | 15/15 PASS,0 ignored |
| `cargo test --locked --lib query_identity_observer --no-default-features`           | 11/11 PASS,0 ignored |
| `cargo test --locked --lib form_observer_shared_schema_tests --no-default-features` | 1/1 PASS,0 ignored   |
| `cargo build --locked --no-default-features --example form_field_map_observer`      | PASS                 |
| Legacy public CLI controls                                                          | 11/11 PASS           |
| Context public CLI controls                                                         | 12/12 PASS           |

Fixtures and captured stdout/stderr are retained. CLI checks cover exact reordered
joins, all three wrong digests, duplicate/missing names and ordinals, conflicting
MainTable, malformed JSON, byte limits and extra arguments. No observer filesystem
mutation or private sentinel egress was observed. No private input, VM, SQL or
native operation was used.

The first local context run passed 14/15 and exposed a test-oracle false positive:
forbidden substring `attribute_name` also matched the fixed public key
`exact_attribute_name_join`. Its log and source pins remain retained. The repaired
test rejects exact raw context keys recursively, retains private-value/XML checks,
and proves private-value and nested-key poison cases fail. Reverse substitution
reconstructed the exact original file digest, proving every byte outside that
test replacement unchanged. Independent source review cleared the custody fix,
projection and oracle repair before the successful bounded retry.

Source manifest 003 SHA256:
`956d70025e4133ce758ce611d4047cea43ed44a3a7985997feb5f19ed1b4771a`.
Retained release receipt SHA256:
`b46325c5b1d833f9a1acb7596854365715e771c2d6986a27f86f6f73bfd1a4b5`.
Local macOS executable: 4,364,200 bytes, SHA256
`83880345036aab0f29e5401bb08b9d2150f4a95c7adb119249be5e680000d391`.
This executable came from the reviewed uncommitted manifest; it is not a Windows
artifact and must not be substituted for clean exact-head CI provenance.

## Remaining gates

A new exact-head Windows artifact, immutable worker/argument binding, public
Windows context/custody controls, sole-operator lease and bounded real-input facts
remain pending. The VM-local context adapter has source-only review; its 21 public
assertions and Windows sharing/ABI controls have not run. Raw context, body and
XML stay inside the VM. Earlier R8/R9 XML-only observations do not provide the
missing query identity or automatic-field provenance.

No native parity, production cause or correction for the remaining UT differences
is claimed. Default chart selection and Form marker behavior remain unresolved.

## Reviewed new-file inventory registration

Offline Quality on `0f826d0b` failed at the unchanged physical-adapter guard:
[run 40093799022 / job 120871748331](https://github.com/Untru/ibcmd-rs/actions/runs/40093799022/job/120871748331).
The reported digest `bf824c6b03bbb50b961e1be2c6f86a7a0462ec6da10f900d84aed573f7f807e6`
is the logical-path hash for `src/mssql_dump/form_body/field_map_observer/context.rs`,
not a content hash. The second new query-identity module was also absent from
the file inventory. The Offline matrix was skipped, not passed.

Two independent static reviews against the unchanged scanner found zero
production UUID, name-routing or XML-policy occurrences in these modules. Their
inline test suites remain covered by the existing test-only rule. The correction
registers exactly those two paths with empty occurrence lists. All 49 previous
records are byte-preserved; every fingerprint/count, scanner rule and exclusion
remains unchanged. No literal allowance or compatibility exception is added; any
future policy occurrence in either file still fails.

Old baseline SHA256:
`93918cc2c2acb688f43673f31007cf18562444fc0c43127de0ef6a2c6e7d00b0`.
New baseline SHA256:
`5d47f1d6f7a768cb4047d8441b3c20ac01b859339a2a627ce1a06d7bed5d4bb1`.
No blanket baseline generation, denied local validator, extracted scanner or
destructive SelfTest was used. Static review and exact JSON/byte-delta checks
are not a runtime policy PASS; the automatic corrective-head CI remains a gate.

The exact `0f826d0b` Windows artifact is retained and verified but not executed.
It remains prior-head evidence until corrective CI and worker/source bindings
are reviewed. No private-input observation or production parity is implied.
