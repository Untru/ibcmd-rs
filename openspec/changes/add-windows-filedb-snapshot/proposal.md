# Windows file-database snapshot capability

## Why
Issue #382's shared partial importer uses WriteSession::snapshot, which still refuses every non-Unix platform despite the existing Windows whole-table writer. The missing token prevents partial preparation and must not be removed without checking the complete Windows publication/recovery chain. MCP integration belongs to #386; SQL/server/import wiring is outside this change.

## What changes
Keep snapshot()->Result<String> and add a narrowly admitted local fixed-NTFS descriptor identity/content fingerprint. Preserve exclusive leaf sharing, held ancestor custody, bounded cancellable reads, security equivalence and recovery semantics. Add clean-room negative and whole-chain Windows contracts. Unsupported profiles fail closed. No universal ABA or atomic filesystem CAS guarantee is added.
