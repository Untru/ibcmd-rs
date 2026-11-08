# ValueList spreadsheet detail — #614, 2026-11-08

## Scope and revisions

Base: `54b796f64da5daed3714acdf7810581fe0e58778`.
Code: `5c1a1faf079784b9d0a11b34972b6b0fa2148e75`.
Public corpus gitlink: `8d138b896f462c3053d16076cbabf0ffe5030c1c` (unchanged).
Rust: 1.95.0. PowerShell: 7.6.9. Builds use the pinned toolchain and a private short temporary directory.

The native `CovSheetValueListDetail` is an empty runtime ValueList: an empty
value-type pattern and `lastId=0`. This change supports that measured profile.
Populated/typed lists, other record revisions, reserved values, final IDs and
unclassified fields continue to refuse. No application-object rule, vendor
adaptation or external dependency was added. The sole dependency-graph addition
is the local `ibcmd-moxel -> ibcmd-schema` edge.

## Failing-first and boundary tests

On the base decoder, the complete public Template.xml comparison failed because
the detail cell disappeared. The independent XML compilation test failed with
`a value of type v8:ValueListType has no writer`.

`cargo test --locked -p ibcmd-rs --test integration coverage_corpus::value_list_detail --no-default-features`
passes all seven tests. They compare complete public XML on both retained corpus
editions, compile/read back the complete document through the existing 2.20/2.21
source projection, preserve a declaration-only ValueList type, and reject unknown
source/storage profiles. Eleven raw mutations pass through complete spreadsheet
admission in both typed and legacy embedded routes. A valid reconstructed public
CF exports the exact expected target; two unmeasured CF profiles exit 2 and leave
that Template.xml absent. No expected XML is regenerated from implementation output.

Namespace controls were first observed red: an unqualified `type` attribute with
the XSI default namespace, a foreign owner namespace, and ordinary `xmlnsUnknown`
attributes on the list and its two children were accepted. Their regression test
now rejects those five cases plus a second namespaced type attribute. The checks
apply to the newly supported ValueList profile; other typed-value branches retain
their existing behavior.

## Matched portable corpus

Both measurements use the identical pinned corpus. The base stored-record and CF
measurements ran in a separate clean checkout of the base revision. Fixed results
were compared by set and variant/path, not just total counts.

| Route | Sets | Base equal | Fixed equal | Remaining differences | Refused |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native CF to XML | 242 | 8060 | 8073 | 5039 -> 5026 | 137 -> 137 |
| Native stored records to XML | 124 | 7449 | 7455 | 1801 -> 1795 | no exit-status change |

All thirteen new equal CF variants and six new equal stored paths are the complete
`CovSheetValueListDetail/Ext/Template.xml` asset (the CF variant also includes its
descriptor). Every remaining per-path stored difference and CF verdict diagnostic
is unchanged. Only those measured gains enter the converter baselines. The public
corpus and prior passing baseline entries are unchanged.

The writer round-trip suite passed four tests and explicitly reported eight #614
refusals resolved. Exactly those eight allowances were removed from
`tests/fixtures/coverage/round-trip.json`.

Reproduction:

```text
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::coverage_sets_hold_their_baseline --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_storage_records::clean_room_corpus_stored_row_route_holds_its_baseline --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features -- --nocapture
```

## Native replay

The clean code revision above was built with `cargo build --locked --profile iter`.
Binary SHA-256: `3f14c92b0c743ee66df7d485c80d769110c22832d8d9142898585bd7596ec4b4`.
The product version is 0.4.1; it does not embed Git SHA, so this proof binds the
clean checkout revision and the built binary digest explicitly.

Live native tools:

| Build | Native executable SHA-256 |
| --- | --- |
| 8.3.27.2214 | `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25` |
| 8.5.1.1529 | `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb` |

For each build, the source is a new copy of the matching public spreadsheet-facets
native tree reduced to Configuration, its Russian Language, and the single
CovSheetValueListDetail CommonTemplate descriptor/body. Only the other template
memberships are removed from Configuration.ChildObjects; all retained file bytes
are preserved. The 1529 live source uses the retained 1150 XML edition.

Every command below ran once per build with `LANG=ru_RU.UTF-8`, a new disposable
infobase and a distinct `--data` directory for every native invocation. All 18
commands exited zero. Import used the default verification, with no `--no-verify`.
`OURS` names the digest-bound iter binary, `NATIVE` the exact build above, and all
paths under `RUN` are new disposable paths outside the checkout.

```sh
$NATIVE infobase create --db-path=$RUN/own --create-database --locale=ru_RU --data=$RUN/data-create
$OURS infobase config import --db-path=$RUN/own --platform=$BUILD $SRC
$NATIVE infobase config apply --db-path=$RUN/own --force --data=$RUN/data-apply
$NATIVE infobase config export --db-path=$RUN/own $RUN/native --data=$RUN/data-export
$OURS infobase config export --db-path=$RUN/own --platform=$BUILD $RUN/ours
$OURS cf bootstrap $SRC $RUN/own.cf --base-free --platform=$BUILD
$OURS cf export $RUN/own.cf $RUN/cf-export --platform=$BUILD
$NATIVE infobase create --db-path=$RUN/loaded --create-database --locale=ru_RU --load=$RUN/own.cf --apply --force --data=$RUN/data-load
$NATIVE infobase config export --db-path=$RUN/loaded $RUN/cf-native --data=$RUN/data-cf-export
```

The complete native/own exports of the same accepted candidate match in **5/5
files on each build, including ConfigDumpInfo.xml**. The complete target MXL XML
also matches the exact public source after native apply/export, own CF bootstrap/
export, and native loading/applying/exporting that CF. Eight comparisons cover
16 complete files with zero differences. XML bytes are compared without
normalization; only filesystem path composition is normalized to NFC.

| Live native target | Complete Template.xml SHA-256 |
| --- | --- |
| 8.3.27.2214 | `9089367b650a8123c46729b55106bbe8c94cf49ebdbb2e35eaa04fb10c9db4e9` |
| 8.5.1.1529 | `87bd4b98189acad1382dc1b5b9f043bb4444e5e62de4ea4a2985135012fa9c25` |


## Gates and cleanup

`cargo xtask affected` selected compiler, MOXCEL, schema and the coverage integration
suite, and required the full gate for manifests/shared fixtures. `cargo xtask check`
passed seven steps: formatting, standalone workspace Clippy, root compilation and
the affected tests (compiler 133, MOXCEL 38, schema 131, coverage 12).
The physical-adapter policy passed without a baseline refresh.

`cargo xtask ci` on the code revision passed **29 steps, 0 failures, 0 skips**,
including all root integration suites, the root library and the empty-temporary-
directory check. Root library: 904 passed, 0 failed, seven existing opt-in tests
ignored. Release-shaped hosted matrix steps are identified below as not run locally.

The separate base measurement target was cleaned (2.2 GiB) and its worktree removed.
All generated native infobases, CF files, native/own export trees and extracted
raw bodies were removed after their hashes and comparisons were recorded. Text
logs, receipts and reduced public input copies remain outside the repository for
review; the feature target remains for the pending independent review.

This is the public coverage regression and its file-infobase/CF proof. It does not
claim a SQL Server, private-configuration, full release-artifact or full-platform
compatibility measurement. Live 8.5 replay uses 8.5.1.1529; 8.5.1.1150 remains an
exact retained public CF/XML oracle, with no live 1150 result claimed.
