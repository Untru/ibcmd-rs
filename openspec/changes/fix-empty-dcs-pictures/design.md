# Decisions

Use the 8.5.1.1529 canonical empty readable Picture for the previously refused
XML 2.21 empty primitive. Its generated namespace prefix follows value depth,
using the same traversal as named pictures. XML 2.20 retains its classic value.
The schema crate owns the empty spelling. No object-name exception or physical
slot is introduced.

The pinned 1150 and 1529 target storage bodies are byte-identical, although
their native XML empty spellings differ. Inferring an exact build from this
body or an unproven archive-version heuristic cannot distinguish them. Adding
a CLI build selector would expand this fix unnecessarily. Rewriting the 1150
oracle would conceal its measured behavior. Neither alternative is used.

Default import verification remains strict: current readable input succeeds;
legacy input that cannot be reproduced byte-for-byte refuses without changing
the database. Unknown attributes, malformed references and inline neighbors
are rejected before the codec returns a rewritten document. A legacy gap
remains open under #618, and separate parameter-name differences under #597
are not counted as equality.

This record documents the measured acceptance adjustment; it does not add a
CLI capability, workflow gate or new complete-file coverage claim.
