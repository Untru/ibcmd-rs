# No-base adopted form interceptors — wave1, 2026-10-01

Scope: #348, E1–E4 on native 8.3.27.2214. BASE is
`2a55cb3462aae8ce7cb8e08e8837bb404154bd68`. Original evidence in
`F:\ibcmd\lab\05\ext` is preserved; new measurements are in
`F:\ibcmd\lab\05\wave1\extensions`.

The common form `ServiceDesk/CommonForms/СвязанныеДокументы` is adopted but
has no BaseForm. The exporter previously assigned Before to every event.
The reader now resolves each event/handler binding using the stored code;
the compiler writes the native no-base event block. Unknown/malformed codes,
ambiguous bindings and unsupported XML spellings fail closed. Failed row
projection removes its source outputs rather than publishing an approximate
form. The existing BaseForm adapter and observed Before command actions are
retained. After/Override no-base command actions remain unsupported.

## Native format

`E = 9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b`, handler
`H = ПриСозданииНаСервере`, body `52bddabb-6ba5-4616-9a71-5392d506cf86.0`:

| Call type | Complete event block |
| --- | --- |
| Before | `{1,E,"H",1,0,E,0,1}` |
| After | `{1,E,"",1,0,E,0,2,"H",1}` |
| Override | `{1,E,"H",1,0,E,2,1}` |

Override uses the first handler's code 2. After has an empty first handler
and an additional handler with code 1. The three verbatim native blocks and
packed/plain row SHA256 values are saved in the fixture directory and
[native-row-evidence.json](form-interceptors-20261001/native-row-evidence.json).
Tests compare the encoded blocks and preserve the complete trailing command
sections. Additional regressions cover shared handler names on distinct
events, malformed tails, unknown codes, conflicting XML annotations, single
quotes, reordered attributes, whitespace around `=`, comments and namespace
bypasses. Noncanonical Events containers carrying unconsumed callType are
explicitly refused.

## Version and measured writes

All three new lab source trees increment the existing four-component Version
**1.7.0.0 → 1.7.0.1**, through the metadata-edit tool, before native import.
No BSL or data metadata changed. Form validation: 0 errors, 1 warning
(callType without BaseForm), 14 checks; the native oracle accepts that shape.
This is a Rust change with lab source evidence. No CFE was built or delivered,
so no built-CFE Version verification is claimed.

Our loader still refuses a direct Version-only root descriptor edit. The
first attempt refused before any write. Following that refusal, each owned
clone was seeded and activated natively with Before and Version 1.7.0.1;
our load then changed only the form handler at the same new Version.
Both After and Override loads report `executed=true`, `compiled_targets=1`,
and only `CommonForms/СвязанныеДокументы/Ext/Form.xml` as changed.
The seed does not establish support for our loader changing Version.

| Measurement | After | Override |
| --- | --- | --- |
| Native staged import/export source equality | 464/464 | 464/464 |
| Our staged load → native export equality | 464/464 | 464/464 |
| Native import → native activation → export equality | 464/464 | 464/464 |
| Our load → native activation → export equality | 464/464 | 464/464 |

Before also has a native staged import/export and our reader equality of
464/464. Every source file in these comparisons matches byte for byte,
including Configuration.xml and Version 1.7.0.1. ConfigDumpInfo is checked
separately: both inventories contain 430 configVersion attributes; only
their values are normalized, and the remaining raw bytes must match. The
own staged comparisons have 177 changed values each. This is a narrower
check than ignoring ConfigDumpInfo. Compact comparison reports are saved
alongside this document; full trees and command logs remain on F.

The baseline reader also preserves all 733 source files of the four existing
8.3.27 extensions (2 + 184 + 464 + 83), with their four ConfigDumpInfo files
checked using the same restricted normalization. No original research DB
was modified. New DB ownership and cleanup are recorded in lab STATUS.md.

## Checks and limits

[quick-gates.txt](form-interceptors-20261001/quick-gates.txt) records fmt,
physical adapter policy, workspace-layer clippy and root library tests on the
final source. The policy change adds only two name-special-case occurrences
in `extension/form.rs`, preserving every pre-existing allowance; no production
UUID allowance was added. No full gate matrix or release build was run here.

An attempted Override re-import onto an already staged After control was
refused by native with a stream-format error. A separate own-Override seed
import exited 1 before creating staged rows; one retry succeeded. Both logs
are retained. An overlapping local test link hit LNK1104 while another test
executable was running; final gates were rerun sequentially after all code
edits. These failed attempts are not counted as successful acceptance.

Actual own staging used the initial corrected encoder build; later changes
strengthened semantic XML dispatch and fail-closed checks. Native applied
twins and verbatim encoder fixtures cover the canonical source used here.
The final source checks include those later regressions. Binary build hashes
and stage reports are preserved with this evidence, without claiming that
an earlier staging binary was the final committed build.

This proves the first measured no-base interceptor case, not all of #348 or
the extension milestone. No 8.5 extension write, drop-in extension apply,
additional command call type, structural change or direct root Version edit
is enabled by this checkpoint.
