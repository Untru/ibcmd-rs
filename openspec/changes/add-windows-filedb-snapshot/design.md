# API and threat model

Base: current master 6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4, including merged #481. This change owns only crates/ibcmd-filedb and its tests. It does not enable or modify src/server/mod.rs, src/file_import.rs, src/sql_import.rs, src/mcp.rs or mssql_source_change source custody.

## Public contract
Existing open_for_write[_with_limits], metadata(), snapshot()->Result<String>, and replace_guarded remain source-compatible. snapshot returns an opaque equality token, not an authorization capability or a cross-platform stable ID. Unix token bytes remain unchanged. Windows uses a versioned domain, handle-derived volume serial/file index and a SHA-256 of the entire locked descriptor. Same content on another live identity is a different token. Callers compare freshly locked sessions before publication, retain their source/policy/final-cancellation checks, and must not infer that success enables unsupported operations.

## Windows profile and descriptor custody
Admit only a rooted DOS drive or verbatim DOS drive reported DRIVE_FIXED and a handle-reported NTFS filesystem. Refuse UNC/network/mapped network drives, non-NTFS (including ReFS/FAT), unknown drive type, identity/volume-query errors, zero file index, directories, reparse leaves/ancestors, unavailable ancestor list/attribute access and unresolved binding. Use GetFileInformationByHandle on the actual retained descriptor; do not substitute pathname metadata identity. GetVolumeInformationByHandleW supplies the filesystem and matching volume serial. The volume root classification is an additional restriction, not identity evidence. No new dependency or credential/access/settings changes.

Windows WritePaths retains each ancestor with FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES and without FILE_SHARE_DELETE, then verifies those identities before operations. FILE_LIST_DIRECTORY participates in Windows sharing bookkeeping; zero-access metadata handles do not enforce the delete-sharing exclusion. Acquisition failure refuses instead of changing permissions. Named revalidation/leaf queries remain metadata-only. The data handle remains read/write with share_mode(0), excluding ordinary external read/write/delete opens while held. Attribute-only named handles compare the original and named file identities without weakening the original handle's sharing restrictions. Snapshot checks binding/profile before and after reading, calls cancellation before and between fixed 1 MiB reads and before returning, and propagates all existing read/policy limits. No cumulative allocation budget is reset or increased.

## Whole publication chain
1. Actual-descriptor policy guard before metadata parsing; measured collation and existing index/budget checks remain.
2. Exclusive original handle plus held ancestor custody; create_new work leaf, denied write sharing, pinned work descriptor.
3. Bounded/cancellable copy, existing Windows owner/group/DACL equivalence and readonly checks before database bytes, verified rows/indexes/unowned pages.
4. Prepare .bak.new by hard link or create_new fallback, retain existing .bak; refuse pre-existing recovery leaves and preserve interruption information.
5. Final caller guard then database/work/recovery identity and security revalidation.
6. Windows drops the exclusive original handle so the existing rename can replace its name. Ancestor custody and work/recovery handles remain retained. Publish data first, then backup; first-rename failure preserves the old database/backup, second-rename failure reports Published and retains both recovery generations.

All six phases require ordinary Windows CI controls before capability acceptance, not only a successful snapshot unit. Existing deny-write/delete, junction, ACL, cancellation and recovery assertions stay intact. New tests cover stable/different-content/same-byte-replacement tokens, active writer/delete/ancestor denials, cancellation/refusal without artifacts, successful snapshot-to-CONFIGSAVE publication and final-guard rejection preserving existing backup.

## Limits
This is an equality fingerprint and cooperating-process exclusion contract. Directory share denials prevent normal directory rebinding while held; no claim covers an administrator, kernel/filter driver, malicious same-user process using pre-existing mappings/handles or drive-namespace changes, deliberate timestamp/identity reuse across sessions, or a malicious leaf substitution after the original handle closes for rename. Pre/post checks do not prove universal ABA exclusion. Per-file replacement is not a transaction spanning database and backup; power-loss durability of directory entries is not newly guaranteed. SACL/audit/mandatory-label preservation remains outside the existing owner/group/DACL check. ReFS/remote-filesystem support would need a separately reviewed identity/profile adapter.

## Documentation provenance
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle (handle identity; network variability).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information (volume/index comparison; ReFS 64-bit IDs not guaranteed unique).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationbyhandlew (filesystem and volume from the actual handle).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew (fixed/remote/unknown classification; trailing root separator).
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew (access/share compatibility and OPEN_REPARSE_POINT).
- https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/82b364ce-6d7b-422f-8d88-4db32eea809a (delete-sharing checks require share-participating granted access; zero-access handles do not qualify).

These documents justify API semantics, not observed Windows acceptance. Initial work is source/design/test authoring only: no Cargo, native platform, VM or Windows runtime result is claimed.
