# Explicit form vertical scrolling

Refs #604. Base `54b796f64da5daed3714acdf7810581fe0e58778`.
Pinned public corpus `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
Code commit `01e4be74a68358aad8227dcf9484addad741e9bf`.

## Rule

The exact public native form-settings record declares root 50 and the
24-member trailer `"","",0,1,"",1,0,0,0,0,0,3,3,0,0,1,100,1,1,0,0,0,{50,0},1`.
Its qualifier/mode pair is 1/1: `use`. The existing other values are 2/2
`useIfNecessary` and 0/3 `useWithoutStretch`; the second value is not a copy
of the first. The schema owns that vocabulary and both bounded slots,
including the already supported optional-block offsets. The writer delegates
parsing and overlay assignments to it, and the native body writer supports
`use` in its existing root-tail encoder. No dependency, vendor change,
generated JSON edit or new numeric production slot index is introduced.

An overlay validates the complete known trailer and original pair before
changing either member. Unknown root/trailer/pair inputs refuse without
partial mutation. Native and XML serialization retain existing modes.

## Failing first and portable evidence

The native tail encoder returned None for `use`; the XML writer rejected
`unsupported Form VerticalScroll: use`; the schema returned None for native
1/1; the public complete Form.xml comparison missed VerticalScroll. Each
regression was observed failing before production changes and now passes.
The exact native tail and full form bytes are asserted, not normalized XML.

- All nine transitions among the three modes preserve the complete layout.
- Unknown pair rejection leaves the original layout byte-identical.
- Schema tests cover known root 50/count 0, root 50/count 1 and root 49/count 1,
  all three modes, unknown pair/root and wrong trailer length.
- `cargo xtask affected` selected form-native, form-schema, form-writer and
  coverage_corpus; `cargo xtask check`: seven PASS.
- Physical-adapter policy PASS without baseline refresh. Schema lookup is
  separated from diagnostic error construction; no physical policy is moved.
- Matched base/fixed full corpus: both 242-set runs PASS; native-proven 13236,
  native-refused 49; equal 8060 -> 8067 (+7), different 5039 -> 5032,
  exporter refusals remain 137. Only those seven new equal variants enter
  baselines; existing expectations and newline conventions are preserved.
- Both base/fixed stored-row suites: four PASS each, existing baselines kept.
- Round-trip suite: four PASS before and after removing exactly eight now
  resolved VerticalScroll writer-refusal allowances. No unrelated allowance
  is removed. Observed-problems suite: PASS.

## Initial binary and native controls

Rust 1.95.0, macOS ARM64, short canonical TMPDIR, debug line tables,
incremental compilation disabled. Initial iter version 0.4.1, built from the
clean code commit above, SHA-256
`800559c2176796a303ed5193459753efcbd3afa4794a58f2e40ec8517c66cdbd`.
The version output does not embed the commit; the clean checkout and binary
hash bind the run.

Fresh native builds: 8.3.27.2214 and approved 8.5.1.1529. Saved 1150 is an
exact offline oracle. The 1529 runtime loads the exact 1150 form-settings CF;
that platform's saved combined CF separately holds the same target regression.
Every native call has unique --data and LANG=ru_RU.UTF-8. All writes use new
disposable infobases; candidate children run with an empty PATH.

Both full-set CF/fileDB readers write a target Form.xml byte-equal to fresh
native, four comparisons in total. Both selected fileDB exports also return
0 and match the complete target form. Source database hashes are unchanged.
The full 1529 set has an unrelated failure described below; those two full
route exit codes are explicitly not called successful.

Additional target-only controls cover all three modes on both builds. Their
six native create/export pairs completed with code 0. Candidate verified
import -> native apply -> native export -> own export completed for all six
cases: all thirty commands returned 0. Together with their twelve native
control calls, the isolated matrix is 42/42 successful commands. Each reader
matches all 24/24 complete files, zero differences; control database hashes
and candidate database hashes around read-only export are unchanged. No
--no-verify option is used. Both source full-set databases also retain their
hashes, including around the two successful selected fileDB exports.

## Reproduced base failure and acceptance limits

Full 1529 form-settings export returns CF=2 and fileDB=255 because the
separate CovTablePictureTitleFont form reaches `TitleFont has no known place
in PictureField`. The same commands were repeated in this worktree detached
at clean merge base `54b796f64da5daed3714acdf7810581fe0e58778`, using a freshly
built debug binary SHA-256
`d59dadf73624ec2703c69524eb4c6dde97a16830d143217eab8d53dd7140b36b`:

```text
cargo build --locked
ibcmd-rs cf export <public-1150-form-settings.cf> <new-base-cf> --source-version 2.21
ibcmd-rs infobase config export --db-path=<disposable-1529-control> --platform=8.5.1.1529 <new-base-file>
```

Base CF=2/fileDB=255, the same single logical entry and exact TitleFont error;
feature outputs retain that failure and still match the target form. The
worktree was restored to the same feature commit in a finally block; the
final iter hash is unchanged. No fix for that other form is bundled here.
Complete full-set import is also outside this proof because BinaryData has
its independent #624 fix. The reader’s root-59 optional-panel recognition is unchanged; overlay support
uses a separately bounded existing core and appended tail. No MSSQL or exact 1150
runtime comparison is claimed.

## Full-gate regression and bounded overlay correction

The first full gate failed the existing metadata-blob legacy root-59 overlay
regression; it is a regression of this change, not a pre-existing failure.
The complete metadata-blob suite subsequently passes all 230 tests. Historical
17-field partial overlays retain only their two previously supported modes;
they cannot admit or overwrite the new 1/1 value. A complete native root-59
36-field trailer validates its core marker and bounded optional blocks before
both assignments. A public stored-body regression, observed red first, now
checks all three modes against the entire parsed native form body, including
module and trailing metadata. This also covers a form with no ordinary child
items. Schema tests cover wrong marker, extra member and partial 1/1 refusal.
The policy gate passes without changing its baseline; the seven-step check
passes. The corrected clean code commit `ff8c84c25f3a466ce0d4f4a7e40b9961f3b11a02` produced final iter
SHA-256 `c05bca9034107d14c24541615fe39b5dbf3838bf2fa8d1cf814e926bfa378342`. The complete matrix was repeated with new
candidate databases and output directories: all thirty target commands
return0, all six cases retain their complete four-file trees (48/48 file
comparisons across both post-apply readers). Full-set CF/file routes retain
exactly the separately reproduced TitleFont refusal on1529; both selected
routes return0 and their complete target forms match. All database hashes
remain stable around reads. Native control references remain unchanged.

## Final gate and review

This report is staged before the full gate so the tracked-file guards see
it. Final staged CI on the corrected code completed29PASS, including workspace,
root integration and root library; existing ignored tests retained.
The earlier failed gate is explicitly recorded above and is not a PASS.
Independent fresh-context review APPROVE at f053eafd: check seven PASS,
round-trip four PASS, metadata-blob230 PASS, whole native form and whole
stored-body regressions PASS. The reviewer independently repeated the final
native matrix in separate databases/output directories: all30 target
commands0,48/48 complete-file comparisons equal and stable database hashes;
full1529 routes retain only the same reproduced TitleFont refusal. Parent
performed the implementation and all original checks recorded above.

## Native matrix hashes

| Build | Mode | Files per reader | Tree SHA-256 |
| --- | --- | ---: | --- |
| 8.3.27.2214 | use | 4/4 | ed7373082225b12e7253f01aff359bd8f91cc75c3edac56e3e56a62a61ca1cd5 |
| 8.3.27.2214 | useIfNecessary | 4/4 | 20f2547ae7915998e96b95ff45fa2a177661b6d847fd4b99c1247c8e6350b255 |
| 8.3.27.2214 | useWithoutStretch | 4/4 | accc93e0fb89d1eb287ce2e996e2ab09b002903be18bf3d559ca4f60344b988e |
| 8.5.1.1529 | use | 4/4 | e3070f71b1ebcd41af0494788ad96d1b9cb4e0f3b4a093fd429feb1eac2f065e |
| 8.5.1.1529 | useIfNecessary | 4/4 | c32681e6c2fce5a4704ffdb27ffb3f7237158a698bd9c123ed7b839404e16eab |
| 8.5.1.1529 | useWithoutStretch | 4/4 | d633a0bfd4bad0b6bf639a0b32418f6254b312c20f85c07c6e2abd0446e89c59 |

Tree hashes cover sorted path, NUL, file SHA-256, NUL, byte length and LF records; ConfigDumpInfo.xml is excluded. No XML or whitespace normalization is applied.
