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

## Historical pending gates (superseded by the retained result below)

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


## Retained original Form acceptance

The separately reviewed R10 sequence completed 45 public controls followed by
one frozen-input observation. The final query projection remained unclassified
where the parser could not establish identity; absence was not treated as
semantic nonselection. R9 had already established a unique direct ordinary
Russian-to-Russian field binding in each affected block.

The first production candidate `11571333` removed the marker but incorrectly
replaced two declared Russian spellings with the automatic English identity.
Its retained failure is preserved. The correction gives a unique direct ordinary
self-binding spelling precedence only after implicit availability succeeds,
mirroring bag Field versus normalized ServerState emission. It does not add all
declarations to the universe, infer a family-specific naming rule, or relax
shadowing, projection, ambiguity and custody guards.

Product `c45d1bb2a9ea260dd5d7bb32f3143eea7ab29076`, PE SHA256
`3f2ae372b411e1c552395ca5745c9e46b31c2d43fe0121ab6fde32e446e4df32`,
was tested by the sole operator against the retained native 8.3.27.2342 reference
using the supported release profile 8.3.27. The original Form is byte-exact:
417,459 bytes, SHA256
`18a5ddb99b013572d9e52dcc19a236d06312cc57d5a906bbcc88205939fac279`.
All 94 ordered UseAlways entries and 109 DataPaths match, with zero differences.
Before/after source fingerprints match; SQL writes, forced termination and private
XML transfer were zero. Shutdown/exit completed naturally with product exit 0.

Sanitized Form result SHA256:
`9b48e235d18cbd7feb3243a2b04954a7a931307ba6c9ba0fe2ba4af145acfcf5`.
Combined physical full-release receipt SHA256:
`3ead95f368c8419a67644b601e82aa3b5a0702fc07f439cad7985edd7ab5ccda`.
The same product's full public export matched 5 of 6 files, including the 43-byte
BSL module and complete ConfigDumpInfo; its remaining Template is not byte-exact.
[Exact product CI](https://github.com/Jimmo910/ibcmd-rs/actions/runs/40151625588)
passed on Windows and Linux.

Local correction gates passed: 9 related Form tests, 9 existing UseAlways tests,
8 XML controls, strict changed-crate clippy, formatting and whitespace checks.
The integrated module donor additionally passed its 3 regressions and production
lib-check. Review found and removed a quadratic namespace traversal before
publication; direct ancestry lookup preserves namespace shadowing semantics.
All fixtures and failed/successful logs remain on the external lab disk.

This is a verified Form-file result, not acceptance of all six original
differences. The five private Template chart-name differences, native publication
context selection and public named-item ordering remain unresolved. Exact-build
registry support, full private-tree parity and new upstream policy-gate results
are not inferred. No scanner/baseline relaxation, denied local validator or
destructive SelfTest was used for this result. Raw body/context/XML stay in VM.
