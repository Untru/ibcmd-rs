# Design: physical-adapter policy guard

## Boundary

The validator scans only committed Rust production source under the explicitly
configured physical-adapter slice.  An item is excluded only when its exact
`cfg(...)` predicate cannot be true with `test=false`; ambiguous `cfg_attr`,
`cfg(not(test))`, and mixed `cfg(any(test, ...))` items remain guarded.
Standalone test/fixture modules are ignored.  `mxl_ir.rs` and `moxel.rs` are
deliberately outside this first slice.

## Inventory model

For each configured source file the baseline records only a repository-relative
file identifier and SHA-256 fingerprints.  A fingerprint is calculated from a
category, normalized source context, and the matched literal.  It never stores
the literal, application names, database content, local paths, or line numbers.
CRLF and bare CR are canonicalized to LF before hashing; whitespace-only,
line-ending and line-number edits therefore do not alter it.

The current inventory must be a subset of the baseline.  This permits deletion
of debt while rejecting a new occurrence, a replacement with a different
literal, or a newly introduced scoped file.  A baseline update is an explicit
reviewable policy decision; it is not an allowlist for arbitrary strings.

## Categories

- `uuid-literal`: hardcoded UUIDs outside test code;
- `name-special-case`: literal branches/comparisons used to route an object or
  metadata name;
- `xml-policy`: direct QName/XML ordering/default-emission literals or decisions.

Schema-owned accessor calls are excluded where their call site contains no
locally chosen literal.  This avoids treating use of the schema corpus as a new
adapter policy decision.

## Failure and report behavior

The validator fails closed for unreadable/malformed baseline, unrecognized
baseline category, a candidate file missing from the baseline, and a current
fingerprint or increased count absent from the baseline.  Baseline fingerprints
absent from current source are permitted reductions.  It reports only category, hashed logical file id, and hashed
fingerprint; it emits no source values, application data, or local paths.

## Verification

Self-tests generate a temporary synthetic repository and mutate it to exercise
pass, added occurrence, changed literal, new file, removed occurrence, test-code
exclusion, schema-accessor exclusion, and malformed-baseline branches.  CI uses
PowerShell Core on Linux and Windows and requires no database, platform install,
or network access after checkout.

## Limitations

The tokenizer joins logical Rust statements after removing comments and only
provably test-only items, and understands normal/raw strings, chars and common
escapes.  XML fragments at immediate sinks and literal concatenations are
fingerprinted conservatively.  It is still not a Rust parser or data-flow
engine: this slice cannot prove that a value denotes a real infobase object, nor
can it infer XML assembled through distant variables or runtime transformations.
The guard should be narrowed or replaced by typed policy ownership as code is
migrated.
