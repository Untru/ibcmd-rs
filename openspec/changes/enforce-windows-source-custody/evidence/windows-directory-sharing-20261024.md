# Actual Windows rename regression and directory-only correction boundary

Published head1eb479cf1d263a0d57b346321061bbaff8158697 / functional2070c830907558ca14ccaf925f6dc43f2a201401 compiled on Windows. Ordinary root job120829882064, run40079496162, reports4192 passed /4 failed /16 ignored. All four failures are Os32 sharing violations at fs::rename:

- windows_directory_anchor_preserves_idle_child_edits_without_leaf_custody: the nested directory is created after RootAnchor opens, and child creation/write/read succeed before nested rename fails.
- windows_detached_compiler_cache_retains_bytes_and_rejects_later_mutation: detached nested ancestor rename fails.
- consumed_cache_rejects_ancestor_rebinding_with_unchanged_leaf: nested ancestor rename fails.
- consumed_cache_revalidates_file_and_ancestor_witnesses: detached leaf rename fails.

These are real failed positive controls, not expected refusals or acceptable frozen-subtree behavior. The same root suite has the two no-leaf/pre-existing-DELETE denials, explicit zero-access negative, three synthetic profile controls, selected-leaf exclusion and two traversal controls passing (9 of the13 scoped controls pass;4 fail). No tests or retained capability lifetimes may be weakened to hide the failure.

The dedicated paired run40079496139 is terminal failure with retained artifact12555720101 (archive digest4914299c83ff5027675601a0cf0bedfeb4818943f0917d60bd2f044b4a33240d). Baseline751 again records2 intended assertion RED +2 PASS. Fixed2070 compiles and passes the two denials plus zero-access control, then fails the idle rename control; later profile/compatibility tests are not run in that paired job. This differs from the broader root suite's executed controls.

The protocol-based explanation is an inference from these results and [MS-FSA FileRenameInformation](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/87f86c9b-6c2a-4803-84b7-131a74a434fa): rename opens DestinationDirectory with FILE_ADD_FILE|SYNCHRONIZE or FILE_ADD_SUBDIRECTORY, aliases of write/append access. Changing directory desired access from zero to LIST while retaining SHARE_READ activates deny-WRITE as well as intended deny-DELETE. This is not an observed kernel trace.

The proposed correction keeps directory desired access LIST|READ_ATTRIBUTES read-only, permits directory SHARE_READ|SHARE_WRITE, and still excludes SHARE_DELETE. Leaf read+SHARE_READ is unchanged. This allows the required destination-parent opens while preserving own-directory DELETE/rename exclusion and pre-existing DELETE conflict. Selected leaf exclusion, full final source census/witness/digest validation, O(depth) transient handles, typed cancellation, budgets and Unix behavior remain required. No handle release, new route, ACL/access grant, fallback or universal ABA/CAS claim is introduced. The unchanged deny and idle/detached controls must all pass through a newly reviewed ordinary CI candidate before acceptance.
