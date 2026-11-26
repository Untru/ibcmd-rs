# Add VS Code object-status preview

Issue #380 needs six directional labels, a minimal object list, Explorer decorations,
folder/Git baselines, changed-only filtering and safe refresh. Reuse the #378 client
and existing server RPC without implementing #379 or a second filedb backend.

This change delivers a reviewable byte-comparison UI/API. Native version semantics
and real filedb acceptance remain dependent on #362/#382 and do not close #380.
