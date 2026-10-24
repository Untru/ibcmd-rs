# Diagnostic validation boundaries

Refs #381. This helper measures structural field-map facts, not native byte
parity. No private source body was used for these checks.

The initial implementation demonstrated a failing-first traversal control by
replacing admitted-record insertion with a drop: the test observed zero groups
instead of one. The saved candidate was restored and verified byte-for-byte
before the green run. Dependency-fetch preconditions are separate from this
behavioral red.

Before the shared-schema migration, the bounded observer tests passed 11/11,
the example built, the external CLI controls passed 11/11, and four existing
Form controls passed. The portable CLI harness also passed 11/11. Fixtures and
logs remain external and retained. These earlier runs are not evidence for later
source changes.

Independent review found and corrected recursive parsing before resource
admission, a quoted-prefix root bypass, and ambiguous secondary map handling.
The reviewer then approved the structural/privacy behavior and the Windows
artifact wiring statically. That review does not establish Windows execution.

The unchanged tracked corpus governance check passed. Initial physical-adapter
policy rejected local token interpretation. Production and diagnostic callers
now share schema-owned query punctuation, DynamicList query properties and exact
zero-marker recognition. The scanner and existing allowances are unchanged.
The new module has zero production policy findings; its new-file scope
registration was independently reviewed and approved as exactly one empty
allowance entry; every existing entry and count is preserved. The unchanged
normal policy guard passed afterward.

Post-migration results: observer tests 11/11, lexical schema test 1/1, four
existing Form controls 4/4, rebuilt CLI 11/11, example build, formatting and
`cargo xtask affected` passed. The latter requests full CI for this source/tool
scope. Chart design construction also validates 11 unmeasured inputs, with zero
native runs and zero expected outputs.

Exact-head CI passed for `9e05b83d037386407f7af0bfc4e6c78291f2c886`, including
Linux/Windows extension checks and the offline platform matrix. The verified
Windows x64 debug observer artifact is 3,637,760 bytes with SHA256
`7a796694762a6826b6b5c8fbe2aa2963a5bf517c53840028447a2aea5dc128e4`.
The sole operator ran it once: exit 0, empty stderr and ten admitted manual
field-map groups. Two groups each had one required DeletionMark candidate,
without duplicate, multiple or shadowed candidates. Their query evidence was
available but had neither wildcard nor exact alias matches. The full typed
universe was unavailable. These anonymous raw groups are not yet bound to the
two affected XML groups, and alias absence does not establish unresolvability.
All five source fingerprints remained unchanged. Private inputs stayed on the
operator host; no source body or application XML is included here.

Steps 4–6 are complete within those bounds. Do not repeat the source-bound
observer because of an old checklist. A later XML-only discriminator requires
its own containment, synthetic-control and live-observation gates.

Pending: `cargo xtask check` and full local CI (the current CI self-test teardown
permanently deletes fixtures), a discriminating production reproduction/fix and
native retest. Remote CI does not substitute for those local checks. No
six-difference acceptance claim follows from this document.
