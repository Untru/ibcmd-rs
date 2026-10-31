# Exact source API, profile and test boundary

Verified master/base: 6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4. Separate branch agent/issue-382-windows-source-custody; #530 and its receipts remain unchanged. The allowed production scope is only Windows directory desired access and source handle/profile identity inside src/mssql_source_change/files.rs and an optional private Windows profile submodule. Isolated tests and this OpenSpec change complete the scope. No file_import/import_plan/SQL/server/MCP/renderer or Unix implementation edits.

## Source API
No public API/type change: HeldSourceRoot::open/open_with_traversal_limits, open_source_operation/open_compiler_operation, source_bytes/read_original_path, fingerprint_paths and require_unchanged keep their ownership and error contracts. RootAnchor and OriginalFile remain private. A successful source capture does not authorize writes. Final source census/witness/digest/policy/cancellation validation stays with its existing consumers.

The profile belongs to the actual source root, directories and regular-file handles, not the database target's profile. Selected OriginalSource capabilities continue to retain their leaf and parent handles. Generic compiler/caller-budget cache entries remain immutable bytes plus detached witnesses: they do not retain one descriptor chain per XML. Reopening for final witness/digest/census validation stays mandatory.

## Windows profile proposal
The admission applies to the canonical root passed to RootAnchor and actual source descriptors. Admit only canonical rooted DOS/verbatim-DOS roots reported DRIVE_FIXED with actual-handle NTFS filesystem evidence. HeldSourceRoot still canonicalizes requested_root first; this change does not establish a new policy for raw operator spelling (for example a drive-relative spelling that canonicalizes to an admitted local root). Internal raw-path tests cannot prove rejection at the public entry point. Preserve canonical_directory's existing per-ancestor reparse checks; do not replace them with canonicalize-then-trust. Reject UNC/network/mapped remote, non-fixed/unknown drive kinds, ReFS/FAT/unknown filesystems, unavailable/inconsistent volume/identity evidence, zero file indices, reparse points, nonregular leaves and multi-link regular files. A source root admission is not inferred from a separately admitted NTFS database target. ReFS would need a separately reviewed 128-bit identity adapter; remote behavior needs its own profile evidence.

The directory branch requests FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES with FILE_SHARE_READ | FILE_SHARE_WRITE, never FILE_SHARE_DELETE. Its desired access remains read-only; write sharing permits the destination-parent add-file/add-subdirectory opens required by ordinary child renames. It does not grant this source capability write access. Leaf .read(true), strict FILE_SHARE_READ and OPEN_REPARSE_POINT remain unchanged. The earlier directory SHARE_READ-only attempt activated an unintended deny-WRITE restriction alongside deny-DELETE and failed the required idle/detached rename controls. Directory acquisition must fail closed if these existing user rights are unavailable; no ACL, privilege, credential or setting is changed. Retain ancestors top-down before resolving descendants, including root and selected nested parents. Actual handle stamps establish NTFS filesystem/volume/file identity for roots, directories and leaves, with same-admitted-volume checks. Path drive classification is only an additional refusal gate, never identity evidence. Canonicalization/metadata inspection can precede admission; refusal must precede inventory payload consumption/publication, not be misrepresented as zero path IO.

## Lifetime, bounds and allowed edits
Keep O(depth) transient handles per capture/read and the existing retained selected capabilities; add no per-XML retained chain. The low-FD 1024-XML/128-descriptor Unix regression stays unchanged. No traversal/count/byte budget reset or increase. Cancellation remains typed and checked at existing read/capture/final boundaries. Unix desired access, inode/content witnesses and algorithms remain unchanged.

A root/ancestor directory cannot normally be renamed/deleted while its admitted handle is retained. This must not freeze ordinary creation/editing or renaming of unpinned child entries, including entries created after capture. Once detached member/parent handles are released, nested compiler-cache edits/renames remain possible; existing final revalidation refuses publication after a change. After all capabilities drop, normal edits and directory renames work again. Selected pinned leaves retain their stricter write/delete sharing exclusion.

This is not a source/target filesystem transaction, unconditional atomic CAS, universal ABA protection, administrator/kernel defense or proof against arbitrary drive-namespace remapping/pre-existing mappings. Identity reuse across sessions and post-final-check changes outside retained capabilities are not newly solved. Do not enable a Windows import/MCP route merely because this helper passes isolated tests.

## First test boundary, before production changes
All fixtures are newly generated owned directories and public short byte strings. Test helpers use explicit TMPDIR or a worktree-local fallback, unique names, no precleanup, and preserve fixtures under IBCMD_TEST_KEEP_FIXTURES=1. No Windows execution or red result is claimed without a real run.

| Control | Existing 6ca behavior predicted from source/MS-FSA | Required behavior |
| --- | --- | --- |
| Empty root/ancestor, only directory handles, rename/delete probes | FAIL: no participating directory access | Sharing violation while held; success before/after |
| Pre-existing ancestor DELETE handle with permissive sharing | FAIL: zero-access acquisition does not conflict | Capture refuses while held; succeeds after close |
| Explicit zero-access directory probe | PASS: metadata query permits rename | Must remain a negative control, not custody evidence |
| Ordinary child create/edit and detached nested rename | PASS | Remain allowed; final census/witness guards still apply |

These tests exercise existing RootAnchor/private source APIs and compile against the pre-change production helper; they must not fail merely because a proposed helper symbol is absent. Existing selected leaf, detached cache, witness/census, reparse/hard-link, traversal, low-FD and cancellation tests remain unchanged. Profile decision-table controls are added with the profile adapter later; real local NTFS positive controls and unsupported-profile refusals must run through ordinary Windows CI after fresh independent review. No compiler/native/VM/SSH or Windows execution is granted in this source/design phase.

## Official provenance
- https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/82b364ce-6d7b-422f-8d88-4db32eea809a (share-access conditions; zero-access handle does not enforce deny-delete).
- https://learn.microsoft.com/en-us/windows/win32/fileio/file-access-rights-constants (directory list/read-attributes rights and file-data aliases).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew (desired access, sharing compatibility and reparse opens).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information (filesystem-specific identity, ReFS 64-bit limitation, reuse over time).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle (remote information variability).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationbyhandlew (actual-handle filesystem/volume evidence).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew (fixed/remote/unknown classification).

- https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/87f86c9b-6c2a-4803-84b7-131a74a434fa (rename opens DestinationDirectory with FILE_ADD_FILE or FILE_ADD_SUBDIRECTORY, which require compatible directory write sharing).
- https://raw.githubusercontent.com/rust-lang/rust/1.95.0/library/std/src/sys/fs/windows.rs (pinned Rust fs::rename uses MoveFileExW; ERROR_ACCESS_DENIED has a separate fallback).
