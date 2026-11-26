# Design

The approved split is by ownership: native inputs, artifacts, generation recipes and provenance belong to Untru/1c-fixtures; Rust test adapters, converter measurements and equality/refusal expectations belong to ibcmd-rs. The submodule is public and uses HTTPS for fork CI. Git records a specific commit, never a moving branch.

The corpus exposes coverage/, coverage-records/, tools/coverage/ and a versioned inventory. Baselines retain their current relative platform/set paths in ibcmd-rs. Source probe definitions and converter diagnostic expectations are joined by stable case ID. Native artifacts are copied byte-for-byte; only documented source-probe splitting and documentation/tool integration edits may change bytes.

A missing checkout, a different HEAD from the indexed gitlink, a changed/missing/extra fixture file or an orphaned baseline fails closed. Existing CI job names, triggers, matrix entries and assertions remain. Native generation tests run from the pinned corpus alongside converter-specific Python and Rust tests. No converter dependency or serialization behavior changes.

Alternatives: a moving remote branch would make old commits unreproducible; copying fixtures into both repositories would create competing sources; moving converter baselines into the corpus would allow fixture updates to weaken regression promises. These are rejected.

Native builds are not repeated because their inputs and oracle bytes do not change. Existing evidence is retained and all hashes are rechecked. Migration evidence includes every source path, its destination, disposition and hashes. A clean clone must fetch the public gitlink without private credentials and pass the corpus checks.
