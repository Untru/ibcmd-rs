# Directory witness API and provenance

## Pinned dependency and ownership
Base: reviewed #533 head `0cb96333e653c79d7f13d7d0200a79833538bcb4`, functional source `63637dbe9914709e57ff64af56b1d8a7fba27a22`. Existing `src/mssql_source_change/files.rs` blob is `fe955f8c0aa6132dccca1607a7c60babff009b76`. Worktree/branch: `issue-386-directory-witness-api` / `agent/issue-386-directory-witness-api`. Only this new API, its private type, owned API tests and this contract are added. The separate census owner changes SourceInventory/capture/revalidation; the separate Windows Plan harness remains pinned and unchanged. Neither #533 history nor its existing tests/workflow is edited.

## Exact visibility and detached type

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectoryWitness {
    root: FileIdentity,
    directory: FileIdentity,
    chain_sha256: [u8; 32],
}

impl RootAnchor {
    pub(super) fn directory_witness(
        self: &Arc<Self>,
        exact_relative: &str,
    ) -> Result<DirectoryWitness, SourceChangeError>;
}
```

All fields remain private. There is no public constructor, raw-field getter, File, Arc, String, Vec or Stamp inside DirectoryWitness. Successful construction proves the opened member was a directory under this seam's checks. The equality record is a constant-size detached value (two existing 128-bit FileIdentity values plus SHA256), rather than a retained ancestor vector. The consumer charges `size_of::<DirectoryWitness>()` and its own exact-path/folded-key/entry bookkeeping before retaining/copying an entry. Exact enumerated spelling is retained once by the census, not duplicated in each witness. This is filesystem custody provenance; synthetic `from_files` or a digest projection must not manufacture it or equate unknown provenance with a known empty census.

## Equality and framing
Root and member identities compare exactly. Ancestry and spelling equality is a cryptographic commitment under the same SHA256 assumption as existing source content digests; it is not equality of retained ancestor vectors. The commitment is domain/version separated, explicitly tagged Directory, and includes the root identity, the checked UTF-8 relative byte length and bytes, ordered identities for every directory component (including the member), and the checked component count. Identity words and framing integers use fixed-width little-endian u64 encoding. No mtime, creation time, length or whole Stamp participates in equality. Changing a file inside an otherwise identical directory must not change this directory-identity witness merely because the directory's timestamp changes.

Input must be a nonempty lexical canonical `/` relative path. Reject absolute, parent/current, repeated separator, non-Unicode, Windows alias/reserved spelling and normalizer-changing inputs before opening any member. The exact spelling is supplied by the census's enumeration and included verbatim in the commitment; this seam does not independently claim a HANDLE-derived canonical name or a raw operator-path policy.

## Traversal and final name checks
Check typed cancellation at entry, before every member-component open/type/identity checkpoint, before name revalidation and before return. `Cancelled` remains `SourceChangeError::Cancelled`. Call root/path revalidation without replacing the retained original root.

On Unix, every component is opened relative to the SAME retained `self.file` (then its opened child), with directory, read-only, NOFOLLOW and CLOEXEC flags. No fresh absolute RootAnchor substitutes for this descriptor-relative member walk.

On Windows, reuse the exact #533 directory opener: LIST_DIRECTORY|READ_ATTRIBUTES, SHARE_READ|SHARE_WRITE without SHARE_DELETE, OPEN_REPARSE_POINT|BACKUP_SEMANTICS. Hold opened parents while opening descendants. Verify every actual handle's directory type, reparse exclusion, NTFS profile, nonzero identity and volume against the retained source root. This remains the existing Win32 path-open/held-parent chain; it is not native HANDLE-relative opening. Existing leaf access/sharing and open_file remain untouched.

Hold the first traversal's directory handles while a second walk rooted at the same original root reopens each edge and compares its actual identity/type with the corresponding first handle. The second traversal needs only a current descriptor, not another retained ancestry ledger. Recheck retained first handles and root/path binding before returning. All temporary handles are released synchronously on success/error/cancellation. Peak member-walk handles are O(depth)+2; existing root/path revalidation can also transiently reopen the absolute root ancestry, so overall handle use is O(root depth + member depth), not a fixed total descriptor guarantee; the detached witness retains none. No timestamp comparison freezes normal child edits.

## Resource and race boundary
The caller must enforce cumulative entry/path/metadata admission BEFORE retained ledger allocation and retain those bounds for recapture. The seam stores only one transient ancestor chain and one constant-size result, so it does not create an unmetered O(directory-count * depth) persistent ledger. The separate census contract decides legacy None traversal compatibility and finite default entry/metadata ceilings; this donor does not silently introduce a depth limit or change existing caller traversal behavior.

The two traversals and final root/name checks are observations, not an atomic filesystem census or mutation log. Namespace races after the final check, transient change-and-restore, inode reuse, reparse ABA and universal source/target CAS are not proved away. Per-component refusal and final census validation remain mandatory. A method without its real census consumer may be unused: integrate the reviewed consumer before compilation rather than suppressing warnings or adding fake usage.

## Tests and acceptance
Owned retained synthetic tests cover stable equality, same-path member replacement, ancestor replacement with unchanged leaf, root/spelling binding, child-edit compatibility, lexical/type refusal, Unix symlink/rebinding controls, Windows real junction refusal, typed cancellation in an isolated child, and repeated low-FD Unix walks. New tests retain all roots unconditionally; production transaction cleanup and existing test teardown are not changed. No platform/native/private fixture is needed. Tests are authored before implementation and remain uncompiled/unrun until a separate combined gate is granted. A missing-method compile failure is not behavioral RED evidence.

The unchanged #533 selected/detached/witness/profile/sharing controls and the Windows Plan strict directory-census controls remain required combined regressions. This seam alone does not establish whole Windows partial import or MCP acceptance.

Official semantics: [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew) documents path-based opens, sharing and reparse flags; [ZwCreateFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-zwcreatefile) documents the separate ObjectAttributes interface. No native API adapter is introduced here.
