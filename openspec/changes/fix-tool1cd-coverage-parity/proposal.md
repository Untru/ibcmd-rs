# Fix tool1cd-backed Coverage parity

## Why

Issue #570 links the public Coverage audit (104 diagnostic signatures and 840 byte-different files) to independently described storage semantics. Existing #545–#551 reproduce missing metadata objects and wrong scalar values. Newer form and template diagnostics need a source-level applicability assessment, not guessed old-format codes.

## What changes

Map every audit class to the pinned tool1cd source and Coverage witness; repair measured shared rules, preserving unsupported-profile refusals. Use the public corpus already reviewed in #535/#565; corpus extraction #566 remains a separate change. Record remaining cases without claiming complete SQL parity or closing unsupported findings.
