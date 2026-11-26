# Extension publication on exact 8.5.1.1150

Measured 2026-10-25 for #578. Native Windows 8.5.1.1150; synthetic public Coverage on independent SQL Server database twins. No private sources or native ibcmd invocation in the implementation.

The native control changes extension Version 1.0.0.1 to 1.0.0.2, an own catalog object module, a managed form group color Tomato to Linen, and a text template. An adopted public catalog remains present. Native CFE build, load, apply and re-export all exit 0.

- Native load leaves exactly ten complete ConfigCASSave rows and leaves the active CAS and registry unchanged.
- Native apply adds five immutable CAS payloads, clears the selected namespace and changes only the registry root, previous rowversion suffix, Version and native update timestamp/rowversion.
- Native does not create an absent dbStruFinal marker on the synthetic twin. A legacy marker already present in ServiceDesk is refreshed; the final own-side repetition reproduces that rule.
- Main configuration, database schema, _ExtensionsInfoNGS, _ExtensionsRestruct and _ExtensionsRestructNGS remain unchanged during this body-only publication.
- Two Params *.ui records change. They are the existing classified licensing/service exception described in docs/apply/params-ui.md; the own implementation does not forge them. Runtime visibility must be tested independently, not inferred from export equality.
- Stored root layout 76 has 77 properties. Its enclosing payload has 29 fields and marker 26. Version is the same schema-owned member as the prior layout68, whose payload has 28 fields and marker25.

The committed native-evidence/extension-apply-851150 fixture binds the complete synthetic native stage, registry before/after and root versions. Full SQL headers and payload snapshots stay in the local immutable lab evidence directory. The original working infobase is never written.

Own-side standard import/apply and a second no-op apply pass on the synthetic twin.
Native re-exports contain 9 identical source files, including changed BSL,
form and template; ConfigDumpInfo.xml differs in four packed storage hashes.
Fresh native and own sessions each report Version 1.0.0.2 from both active-session
and database extension lists and execute the new BSL result `changed-B`.

A separate real ServiceDesk twin passes four successive publications:
1.9.153.0 -> 1.9.153.1 -> 1.9.153.2 -> 1.9.153.3 -> 1.9.153.4. Each final XML, native-built CFE,
native-after and own-after Version is 1.9.153.4. All 827 source files agree,
including all 307 BSL files. The raw 828-file comparison has exactly one
ConfigDumpInfo.xml difference. Fresh sessions report 1.9.153.4 from both lists.
Main Config/ConfigSave, both unrelated registry rows, NGS/restruct registries,
change registrations and old CAS payloads remain unchanged. The existing
legacy marker is refreshed by both implementations.

Storage is deliberately reported separately: the last native ServiceDesk apply
adds 242 packed CAS rows while the verified overlay adds 3. Native Params UI
and Files mobile/user-cache records also change; the own writer leaves them
alone. Their source semantics are not inferred from raw byte equality, and no
general compressed-storage or service-cache parity claim is made. Independent
native re-export and fresh-session visibility are the acceptance controls.

An injected CAS collision (57204), after earlier inserts in the transaction,
leaves all 14 SQL snapshot tables unchanged. A stale registry rowversion (57201)
also refuses the saved plan with all 14 snapshots unchanged. A native-staged
Catalog CodeLength change is refused by the schema-safety preflight with all
14 SQL snapshots unchanged. Unsupported-build,
unknown-layout, digest drift and unmeasured-mode refusals are covered by tests.

The same synthetic control was repeated on exact 8.3.27.2214. Its native
80327 manifest and complete registry transition are committed separately.
Standard import/apply and no-op apply pass; all 9 source files agree, with
ConfigDumpInfo.xml the sole raw-file difference. Both fresh sessions report
1.0.0.2 and execute `changed-B`. Main Config/ConfigSave, DBNames/DBSchema,
physical schema, NGS registry, change registrations and ExtProps stay intact.
The native 27 apply additionally adds two empty restructuring bookkeeping
records and one empty NGS scratch record; the own writer preserves these tables.
These are reported, not hidden by the source comparison; see the existing
scratch-state classification in docs/apply/own-apply.md and native-apply-trace.md.
No extension data-table restructuring is claimed.

Local verification: `cargo xtask check` passes all 6 steps; `cargo xtask ci`
passes all 27 steps. Root library: 4142 passed, 0 failed, the existing 16 ignored
remain unchanged. Standard CLI and platform boundary controls pass 12 and 10
cases respectively. No failure is dismissed as pre-existing and no baseline is
weakened. Release build, CI native micro-corpus parity, SBOM and release audit
remain CI-only checks. Independent fresh-context review of 226cf5b1 returned APPROVE with no findings.
The reviewer independently repeated check (6 passed), extension library suites
(62 and 163 passed), raw source comparisons, preservation and refusal evidence.
The author repeated all three commands after approval. Only this evidence and
task-completion documentation change after the reviewed implementation.

The final exact-27 repetition changes 1.0.0.2 -> 1.0.0.3 and `changed-B` ->
`changed-C`, with 9/9 source files equal and both fresh sessions observing both.
Native CFE re-export confirms the source versions on both platforms.

Commands used (disposable database and evidence paths omitted):

```text
cargo test --locked -p ibcmd-rs --lib exact_851150_extension_write_is_admitted --no-default-features
cargo test --locked -p ibcmd-rs --lib --no-default-features
cargo test --locked -p ibcmd-rs --test integration dropin_cli:: --no-default-features
cargo test --locked -p ibcmd-rs --test integration platform_oracle_boundary:: --no-default-features
cargo xtask check
cargo xtask ci
ibcmd config import --extension=CovCombined --out=<new.cfe> <source>
ibcmd config load --extension=CovCombined <new.cfe>
ibcmd config apply --extension=CovCombined --force
ibcmd-rs infobase config import --extension=CovCombined <source> <verified SQL/RAS options>
ibcmd-rs infobase config apply --extension=CovCombined --dynamic=disable <verified SQL/RAS options>
ibcmd config export --extension=CovCombined <new-empty-directory>
```

The first command fails before admission with the explicit unsupported
`mssql.extension.write` capability, and passes after implementation. Initial CI
runs also caught obsolete standard-CLI refusal assertions; these now assert the
new pre-I/O target guard, its exit code and complete output.

Historical measured binary and raw tree hashes:


The own-side initial attempt refused the modified Linen form before SQL writes. The same three-color dictionary fix already measured in PR #574/code94634ce6 is reused in the extracted ibcmd-form-native crate (master moved the original module). Its public Coverage controls and exact inverse tests remain the authority; no alternative color heuristic or application special case is introduced.
The second own-side attempt also refused before staging: the SQL extension export still uses its legacy color reader for this form body. The three independently measured color values are therefore carried consistently by that existing reader as well. Both refusals are preserved; source verification is never bypassed.

| Control | Tested binary SHA-256 | Native raw tree SHA-256 | Own raw tree SHA-256 |
|---|---|---|---|
| Synthetic 8.5.1.1150 | 32ce6611be7edcfb88a77fbc19a09a9821a9e6b80bc95b82cd90fa9f4f3c6a63 | 2c9860283c20b207a4d8560f0457dbe65c34f7d0df70cad7725732a2a0157908 | 870e9bcb215e2e7539c09d139cf12e76a9691ffee03393872e66104335822953 |
| Synthetic 8.3.27.2214 repeated | 42b7d442bfb1eb16383212eccfdcb7bb8e244a729d6fd7dbddc6c0dcf49eed8b | aac54fafa2c162702efd58b8bb27639e6f313a8eaf56f503b593605b65c9428e | fbe443ab05bf3f141031f3ac86eadcc896cd86e2220704a32d334b83e200d73f |
| ServiceDesk 8.5.1.1150 final | fb859eb4d65a6fe18c4820725d7b2578a02908649def16c378e39df039bf8c31 | eb9450a2e876f4dd2ac8f75725643af01098ef20c7f85d957825a1f3f8b02014 | 5cfe6890263341d08d093ebdb09a8b5726abbc22699cf985fddabda618d67ae3 |

Different raw tree hashes include ConfigDumpInfo.xml; source comparisons above remain byte-exact. Full command receipts, the captured source tree and SQL snapshots are retained locally.


## PR review follow-up

The external review exposed two gaps after the initial approval: recovery
restored the prior CAS root while retaining the published Version, and the
8.5 profile admitted legacy staging formats without exact-build evidence.
Both new regression tests fail on 6d6b248f: recovery reports 1.0.0.2 instead
of 1.0.0.1, and format 80310 is admitted for the 8.5 profile.

Recovery now copies the prior encoded Version field into the restored
registry envelope, preserving the current optimistic rowversion and every
other field. Complete native-fixture envelopes are checked on both exact
builds. Empty, UTF-8, maximum-length and malformed encoded fields have
separate codec controls; unsupported UTF-16 Version encoding remains refused
(the registry metadata has its independent UTF-16 support).

Exact 8.5.1.1150 publication admits 80327 and 80501. The 8.3.27.2214 format
set is preserved. This write gate does not narrow standalone container reads.
The CLI documentation now distinguishes unconditional pid/remote refusals
from the main-configuration sqlcmd refusal.

The original SQL/native source comparisons above remain historical controls
of the import/publication paths. These follow-up fixes change recovery and
write admission; no new application database is used for their tests.


The follow-up regression suite passes all three cases. The previous remote
Linux/ARM/macOS failures were isolated to the release audit's obsolete
unsupported-extension expectation; compilation, integration, root units and
native micro-corpus had passed. The audit now requires the supported import
and apply commands to fail their target-binding guard with an empty PATH,
while unknown platform operations still require exit 1. Python controls fail
before that change and pass afterward (17 tests); the actual pinned executable
also passes the complete clean-PATH boundary audit. Full final gate results
supersede the original verification counts when completed.
