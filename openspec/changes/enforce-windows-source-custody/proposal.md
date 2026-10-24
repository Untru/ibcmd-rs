# Enforce the bounded Windows source-custody contract

## Why
The shared source helper used by #382 and selected-source/compiler consumers opens Windows directories with access_mode(0) while claiming deny-delete ancestor custody. MS-FSA excludes zero-access handles from those share-access checks. Selected leaf denial can hide this defect in a combined ancestor test. The same helper also uses a 64-bit file ID without explicit NTFS/local admission, so the stronger identity claim cannot cover arbitrary ReFS/network profiles.

## What changes
Keep HeldSourceRoot, OriginalFile/witness, source-byte and final-revalidation APIs unchanged. Establish share-participating directory-only access with directory READ/WRITE sharing but no DELETE sharing, retaining unchanged strict leaf READ sharing. Admit a reviewed local fixed-NTFS source profile using actual source descriptor evidence, independently of the target database. Preserve selected retained handles, detached immutable caches, O(depth) transient custody, Unix behavior, traversal bounds and typed cancellation. Add isolated no-leaf, pre-existing DELETE, zero-access negative-control and idle-edit fixture tests before production changes. This is separate from filedb PR #530 and from neutral planner/SQL/MCP wiring.
