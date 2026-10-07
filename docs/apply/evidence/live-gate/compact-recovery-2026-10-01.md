# Compact standalone recovery files

The main-activation recovery mirror stores binary preimages in an adjacent
content-addressed pack rather than JSON arrays of decimal bytes. Both the SQL
activation path and the exclusive own-apply mirror publish format 2 before the
executor runs. The exclusive executor's separate `config_apply.recovery_dir`
remains authoritative for its wider folding/cache operations.

The manifest binds database, mode, generations, the historical snapshot token,
all row headers and pack references through its payload digest. The reader
checks bounded regular files, the exact adjacent basename, whole-pack digest,
contiguous row ranges, per-row digests, complete pack coverage, row budgets,
duplicate keys and the reconstructed snapshot token. It executes no SQL and
does not establish database ownership, a trusted signature or safe undo.

Limits remain 128 before-image rows and 128 staged rows, 16 MiB per row and
32 MiB for each set. The two marker preimages have separate per-row bounds;
the entire pack is bounded at 96 MiB and the manifest at 2 MiB. Input refusal
occurs before either file is published. The synchronized complete pack is
published first with no-clobber semantics, then the manifest. Readback must
equal the admitted snapshot before the caller can execute SQL. If manifest
publication fails, an unreferenced complete pack may remain; no automatic
cleanup removes it. Directory power-loss durability is not claimed.

Preserve the pair together. Existing standalone numeric-array files remain
historical evidence and are not silently rewritten or treated as format 2.
The separate LIVE checkpoint reader, embedded snapshot and token bytes stay
compatible with format 1; their numeric arrays are not compacted by this change.
Generic guarded undo and compact LIVE checkpoints remain open parts of F8.

The later [opt-in compact LIVE envelope](compact-live-recovery-2026-10-01.md)
adds a separate format-2 representation and legacy reader compatibility.
It preserves the default format-1 branch; actual format-2 cycle acceptance
and generic guarded undo still require their own evidence.

Regressions exercise exact header/byte roundtrip and identical repeat;
conflicting publication; missing, truncated and altered packs; rechecksummed
overlapping references, invalid tokens/database, foreign pack paths and budgets;
invalid inputs and manifest/pack path collision before publication. Coordinator
logs record focused checks, mandatory quick gates and immutable peer review.
