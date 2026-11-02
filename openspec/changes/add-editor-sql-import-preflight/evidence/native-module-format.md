# Public SQL native module formatting

The retained full export receipt `811808513f267f44a801bed23d33ad9943e4545c845e8ce64055666b7d7181f6` compares product `1157133354b14a012e9c3e8fd9bbe58b2bef01b2` with the existing native 8.3.27.2342 public fixture. Product profile is the supported release `8.3.27`; exact-build registry support is not claimed. The product exited naturally with code 0, performed no SQL writes or native calls, and preserved source fingerprints. Four of six files matched exactly, including all five CDI tuples and versions.

`CommonModules/Portable/Ext/Module.bsl` was 38 bytes with LF and no BOM; native was 43 bytes with UTF-8 BOM and two CRLF line endings, SHA-256 `f6c91e7f53f80656a0a726cf13fc3ae8fdfd54ace2fbd26069885a101b611e48`. Removing only BOM and normalizing line ends made these two modules equal. Existing checked-in native-export-command module fixtures also use BOM and CRLF. The stored text decoder intentionally preserves its text-member bytes; this is an output-format issue rather than evidence of lost BSL content.

The narrow fix formats modules at filesystem source emission only. It preserves an existing BOM/CRLF, adds no final newline, and leaves lone CR and all other bytes unchanged. Raw storage/debug exports and memory sinks used by the editor/compiler retain their original bytes. Only files already selected for emission are affected; no post-export directory rewrite is performed.

The three focused writer regressions cover the exact 43-byte public native oracle, Unicode and idempotence, and raw/memory preservation. All three passed on Rust 1.95 with jobs=1 (`logs/issue-382-native-bsl-tests.log`); the filtered run executed only these three tests, retained their fixtures, and passed a subsequent root lib-check (`logs/issue-382-native-bsl-check.log`). Independent source review found no blocker. A fresh Windows product export remains necessary before claiming this native difference is closed.

The seven ChartPolicySeed template text differences are separately owned by #529 and are not changed here. This fix does not establish full-tree parity, SQL import authority, same-session CAS, or publication readiness. Raw SQL rows and native XML remain in the VM.

The full donor branch `issue-382-sql` (parent `d9d0f8ef`, not the integrated #529 branch) physical-adapter guard currently refuses the inherited, unchanged `src/mssql_dump/import_evidence.rs` donor as an unbaselined file (path SHA-256 `5574a62490168322556d12d020d387ae765a939f0f12587067ea69e930825999`). That separate gate was reported to its owner; no baseline was edited or check suppressed. This three-file module-format donor excludes the observer. The guard self-test was not run locally because it permanently removes its temporary fixtures, contrary to the lab retention constraint.

## Filesystem validation after root-suite consolidation

Exact-head CI for `2ca6d892` exposed two filesystem expectations that still
required BSL without BOM (and LF in the conversion-matrix case). Both expected
files now use literal BOM+CRLF bytes; raw packed-module comparisons remain
unchanged. The tests exercise both module-only/compiled load routes and both
Format15/Format16 conversion routes.

The compiled load arm also reproduced an internal verifier refusal before
publication: its edited source permits optional BOM, while its re-export uses
the native filesystem writer. The verifier now projects only expected `.bsl`
bytes with the shared native-file policy, preserving exact-byte acceptance and
the existing XML structural rule. Actual output, edited files, packed payloads,
object ownership and CDI are unchanged. Negative unit controls reject changed
BSL content, malformed projected output, and projection for non-BSL paths.

Both focused integration tests passed after this fix. The verifier unit,
packed-codec/compiler policy tests, CLI refusal tests, Form alias tests and
module/status/stage controls also passed (24 root unit tests in total).
`ibcmd-moxel` passed all 45 unit tests, including the relocated typed policy
controls; its all-target clippy check passed with warnings denied. These are
local Rust tests, not a new native or SQL parity measurement. Retention mode
keeps the edited/converted public fixtures, including the replaced closed
module input, without weakening assertions.
