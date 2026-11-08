# DCS parameter names and configuration ScriptVariant — 2026-11-08

Refs #597. The change is independent of the other coverage fixes and starts at
`54b796f64da5daed3714acdf7810581fe0e58778`. Code, regression tests and the 353
baseline additions are committed as `e3904346d59ec6ff933bcfbc547b004b5f1c1049`.
The pinned public 1c-fixtures revision is
`8d138b896f462c3053d16076cbabf0ffe5030c1c`.

## Rule and boundaries

Native export chooses the spelling of standard DCS keys from the
configuration's `ScriptVariant`, independently of the process locale. The
English configuration writes `Title`, `BackColor`, `ChoiceForm` and the other
measured built-in names; the same configuration with Russian ScriptVariant
writes their Russian names. The existing configuration-root schema supplies
the flag. Selected DCS template export requests that metadata too.

The schema vocabulary contains 69 native-measured pairs. The XML pass changes
only core `parameter` children of core `item` in standard appearance, output
and input parameter containers, resolved by namespace URI. User data
parameters, values, unrelated namespaces and surrounding bytes are retained.
Unsupported nested/opaque content in a standard key is refused. Tests also
hold the UTF-8 BOM offset, which differs from quick-xml's input positions.
No vendor snapshot, generated schema JSON or dependency changed.

## Failing first and offline checks

- `coverage_corpus::english_composition_parameter_names_match_native` failed
  before the fix: the native minimal template contained `Title`, while the
  converter emitted `Заголовок`. The final regression compares all bytes of
  44 template files from `corpus-english-composition`,
  `corpus-english-parameter-contexts` and
  `corpus-structured-composition-contexts`, for pinned 8.3.27.2214 / 8.5.1.1150
  inputs. All 44 now match.
- The selected-body index regression first failed because it did not request
  configuration metadata. It now passes.
- Four focused XML tests pass: standard contexts versus user names and
  values, namespace identity, complete BOM-bearing UTF-8 output, and refusal
  of nested/opaque key content.
- `cargo xtask affected` selects `ibcmd-dump`, `ibcmd-schema`, `ibcmd-xml` and
  `coverage_corpus`. `cargo xtask check`: 7 PASS. Dump tests: 1273 passed,
  1 existing ignored; schema: 130 passed; XML unit tests: 343 passed,
  1 existing ignored, plus integration tests 7 passed / 1 existing ignored
  and 3 passed; coverage: 6 passed.
- The staged physical-adapter guard passes without a policy-baseline change.
- All 590 DCS name predicates in the unchanged historical observed-problems
  ledger are now equal. The complete structured templates containing them
  are held by the strict byte regression. The remaining 656 observations
  retain their recorded symptoms.

An initial sandbox check failed at seven PostgreSQL test socket binds with
`Operation not permitted`. The unrestricted PostgreSQL subset passed 9/9,
then the complete unrestricted check passed. The failed sandbox invocation
is not counted as a passing check.

## Full public corpus measurement

Both base and fixed runs used:

```text
cargo test --locked -p ibcmd-rs --test integration --no-default-features \
  coverage_corpus::coverage_sets_hold_their_baseline -- --nocapture
```

| Measure | Base | Fixed |
| --- | ---: | ---: |
| Sets | 242 | 242 |
| Generated variants | 13285 | 13285 |
| Native-proven variants | 13236 | 13236 |
| Platform-refused variants | 49 | 49 |
| Byte-equal variants | 8060 | 8413 |
| Different variants | 5039 | 4686 |
| Exporter-refused variants | 137 | 137 |

Only the fixed-minus-base improvement, 353 variants across 15 baseline files,
was added. Previously equal unlisted variants were checked separately and
none was lost. Per-set exporter-refusal counts remain unchanged in all 242
sets. Existing baseline entries and original CRLF formatting are preserved.
The combined sets improve by 40 variants on 8.3.27 and 39 on 8.5; this is a
scoped improvement, not a claim that their complete trees match.

## Fresh native controls

Fresh native builds: 8.3.27.2214 and 8.5.1.1529. Native 8.5.1.1150 was not
executed; its pinned public files remain exact offline oracles.
All native calls use `LANG=ru_RU.UTF-8`, disposable file infobases and a
separate `--data` directory for every call.

For each build, the public English parameter, structured composition and
nested composition sets were loaded and exported. Counterexamples import
the same public XML tree with only `ScriptVariant` changed. English XML
imports reproduce all 76 template files of the corresponding English CF
controls, removing CF/XML input-route ambiguity. The paired English/Russian
controls authenticate 998 standard-key occurrences per build, 1996 pairs in
total, and the same 69-name vocabulary on both builds.

A separate clean-room control declares a user data parameter named
`Заголовок` beside the standard title output parameter. On both builds and
both ScriptVariants the user name remains `Заголовок`; only the standard key
changes to `Title` for English. The exact minimal CF from
`corpus-english-composition` was also replayed on both native builds.

Commands, with new scratch destinations for each invocation:

```text
native ibcmd infobase create --db-path=<new-db> --create-database \
  --locale=ru_RU --load=<public-set.cf> --apply --force --data=<unique-1>
native ibcmd infobase config export --db-path=<new-db> <native-tree> --data=<unique-2>
native ibcmd infobase config save --db-path=<new-db> <native-saved.cf> --data=<unique-3>

ibcmd-rs cf export <native-saved.cf> <new-ours-cf> --source-version <2.20-or-2.21>
ibcmd-rs infobase config export --db-path=<new-db> --platform=<exact-build> <new-ours-file>
```

For XML controls, `--import=<tree>` replaces `--load=<cf>` during native
creation. Candidate readers run with an empty PATH and compare complete
Template.xml bytes without normalization.

The final `ibcmd-rs 0.4.1` iter binary was built from the clean code commit
`e3904346d59ec6ff933bcfbc547b004b5f1c1049`; SHA-256:
`d904316636dae6cb737b731820dc0cdda911cb78539c10f6a66ccb8fa2d1536e`.
Native executable SHA-256 values:

- 8.3.27.2214: `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`.
- 8.5.1.1529: `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.

All 64 native create/export/save control commands returned 0. Final candidate
replay used 36 CF/fileDB routes: 32 returned 0, while four extra nested 8.5
routes retained the separately reproduced picture refusal described below.
Across these outputs, 312 complete template files match native byte for byte;
four copies of that picture template are absent. No differing emitted
template remains in this control matrix. Every source 1CD hash is unchanged.
The four nonzero routes are not counted as completed exports.

Selected object export was repeated for both ScriptVariants on each build:
4/4 complete output-parameter templates match, commands return 0, and source
1CD hashes are unchanged. CF export under `C.UTF-8`, `en_US.UTF-8` and
`ru_RU.UTF-8` matches the exact minimal native template in 6/6 cases, all
exit 0. These checks use the same final iter binary and an empty PATH.


## Remaining acceptance limits

The extra 8.5 nested composition controls retain a refusal on an empty
`v8ui:Picture` in the standard-pictures template: `a picture value without a
picture reference`. Both CF export (exit 2) and file export (exit 255) fail
there for English and Russian. The exact four commands were repeated on a
clean merge-base checkout at `54b796f6`; the failed entry and message match
those of the candidate, and the databases remain unchanged. Other template
files in those runs are compared separately. These four routes are not
reported as successful complete exports.

Complete combined-tree equality remains outside this independent fix;
other recorded coverage differences are retained. MSSQL, EDT runtime,
release packaging and private configuration runs were not executed here.

## Full merge gate and failure accounting

After staging every new file, `cargo xtask ci` completed with **29 PASS**, no
failed or skipped steps. The final run reports:

- Workspace crates: 4710 passed, 0 failed, 89 existing ignored.
- Root integration suites: 554 passed, 0 failed, 4 existing ignored.
- Root library: 904 passed, 0 failed, 7 existing ignored.
- The isolated temporary directory is empty after the run.

The first full CI invocation did fail: 25 steps passed, the workspace step
failed, and three subsequent steps were not run. Its apply-check unit binary
reported 127 passed, 2 failed and 6 ignored. One record-format test could not
read its temporary Configuration.xml; another classified a string widening
as an uncovered attribute property. That failed invocation is retained and
is not counted as a successful gate.

Without changing production code or tests, the focused candidate crate
repeat passed 129 tests with 6 existing ignored. A separate clean detached
worktree at `54b796f64da5daed3714acdf7810581fe0e58778`, with its own target,
ran `cargo test --locked -p ibcmd-apply-check --lib`: five repetitions each
passed 129 tests with 6 existing ignored; an additional uncaptured-output
run also passed. The original two failures were not reproduced there, and
their cause is not established. They are not labelled as proven pre-existing
failures. The complete gate was then rerun successfully on the unchanged
code. The temporary base worktree and its target were removed after the
audit; the candidate code commit and iter binary hash remain unchanged.

## Independent final review

Fresh-context critic: **APPROVE** at
`022a80048e9a5baac533233f4f2ec3ef0c3c9dc8`. Independently reran all seven
`cargo xtask check` steps and the observed-problems test (590 equal / 656
known symptoms); replayed all 36 CF/fileDB controls (312 complete Template.xml
files equal, four unchanged empty-picture refusals, source hashes unchanged),
four selected exports and six locale controls. Four additional native exports
from fresh disposable database copies compared 28 templates exactly. The
reviewer authenticated all 69 dictionary pairs against paired native trees
and verified 353 baseline additions with no removals.

Parent verification read the production diff and reran the four XML-pass
unit tests, the selected-index regression and the exact 44-template public
corpus regression: 4/4, 1/1 and 1/1 PASS. The four documented empty-picture
refusals match the symptom and exact variant of #618; that fix is excluded
from this independent PR. This final evidence commit changes no code,
tests or baseline.
