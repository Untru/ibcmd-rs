# Native numbering of anonymous form services

Refs #596. Base `54b796f64da5daed3714acdf7810581fe0e58778`.
Pinned public corpus `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
Code commits `11ca6a7361ce8d4427e9f21eacf4f4e3db1fe1b1` and
`8da9c3129d639de1e09cc72a79c534fed91bf7c1`.

The anonymous-item numbering pass traverses actual structural members in
publication order: core services, UUID-prefixed regular children, then the
appended services of 8.5. The converter's existing revision table owns those
tail bounds. It preserves explicit IDs and copies the original text outside the
replaced identity tuples. Quoted text contributes neither anonymous identities,
maximum IDs nor hidden navigator records. The existing hidden navigator rule
is owned by form-schema; the physical-adapter policy baseline is unchanged.

## Failing first and portable checks

The public regression compares the complete sorted `(tag, name, id)` vectors
of four native forms for each exact 2214/1150 fixture. Before the fix the group
tooltip received 5 rather than native 3, and the child's services received 3/4
rather than 4/5. All eight maps now agree. Four unit regressions independently
failed before their fixes: container/service order; identity-like quoted text;
a quoted high explicit ID; and a quoted hidden-group header. Their assertions
compare the complete transformed layout.

Rust 1.95.0, macOS ARM64, canonical short `TMPDIR=/private/tmp`, debug line
tables and incremental compilation disabled. `cargo xtask affected` selected
form-body, form-schema and coverage_corpus. `cargo xtask check`: six PASS;
form-body: 575 tests passed (including conversion-admission coverage). Physical-adapter policy PASS without refresh.

Matched base/fixed `coverage_sets_hold_their_baseline --nocapture` runs both
passed for all 242 sets. Generated 13285; native-proven 13236; native-refused
49. Byte-equal variants 8060 → 8358 (+298); different 5039 → 4741; exporter
refusals 137 → 137. Only the fixed-minus-base equal entries were added to
baselines; prior expectations and existing newline conventions were preserved.

## Native checks

Fresh native builds are 8.3.27.2214 and the agreed 8.5.1.1529. Saved 1150
fixtures remain an exact offline oracle; fresh runs are not labelled 1150.
Each native invocation has unique `--data`, `LANG=ru_RU.UTF-8`; all writes
are confined to newly created disposable infobases.

```text
ibcmd infobase create --db-path=<scratch>/ib --create-database --locale=ru_RU --load=<public-context.cf> --apply --force --data=<scratch>/d1
ibcmd infobase config export --db-path=<scratch>/ib <scratch>/native --data=<scratch>/d2
ibcmd-rs cf export <public-context.cf> <scratch>/ours-cf --source-version=<2.20|2.21>
ibcmd-rs infobase config export --db-path=<scratch>/ib --platform=<native-build> <scratch>/ours-file
```

Final iter binary SHA-256:
`2d7484129acf8e2fddd183d8c3b86dca4ff091efe9cd525c1d7efc164bc80c4d`,
version 0.4.1, built from clean code commit
`8da9c3129d639de1e09cc72a79c534fed91bf7c1`. The version output does not
embed the commit; clean-checkout provenance and the binary hash bind this run.

The minimal context set passes all 16 complete named-ID maps through CF and
fileDB on the two builds. Twelve of those sixteen complete Form.xml files
are byte-equal; four 1529 table files retain the independent empty
AutoCommandBar spelling difference (#594). Both source database hashes are
unchanged.

Additional fresh native controls load stored-context-panels on 2214 and 1529,
and navigation-commands on 1529. All six native create/export commands return
0. All six candidate CF/fileDB exports return 0, the twelve targeted service
ID comparisons agree, and every common named item ID agrees in all forty
compared form files. All three source databases retain their hashes. Missing
or extra items in these stored-panel forms are reported separately; complete
form parity is not claimed.

## Appended services and conversion admission

Two further red-first public tests hold the stored-column service IDs
(15/16 instead of 23/24, three pinned profiles) and all twelve navigation form
ID maps (including Back tooltip 6 instead of 5 and journal Create tooltip
23 instead of 15). Both now pass. A raw 8.5 service in an appended tail can
consume a number even when conversion removes the record. Numbering therefore
runs on the original tree before conversion. The assigned-ID set lets the
converter retain its former treatment of originally anonymous records; it
does not admit their unmeasured property-bag shapes as explicit records.
The same explicit unknown record still refuses, held by a whole-node unit
test. Trailing records retain their separate conversion context.

## Complete emitted-form ID audit and limits

The final iter audit reads all 227 pinned configuration-CF archives; 71 contain
native forms. Across 12661 emitted Form.xml files, no common `(tag, name)`
identity has a differing ID. An intermediate two-phase candidate had 34 such
forms / 66 differing IDs; the three-phase rule removes all of them without
changing the measured full-file count (the remaining files have other gaps).

Ninety-three native forms were not emitted and are excluded from that
claim. Another 765 emitted forms have missing or extra named identities;
these structural differences are also excluded from an assertion of complete
identity-map or whole-file parity. The common-ID comparison does not hide
those counts. Stored optional root panels also expose an existing 8.5
discriminator boundary; that structural admission is not broadened here.
No missing form or missing item is declared compatible by this audit.

The change uses Refs #596: it resolves the measured numbering differences,
while complete trees and unexported forms remain outside this proof. No
MSSQL or exact 1150 runtime run is claimed. No private/native artifacts or
application source trees are committed.

## Final gate and review

The full gate is run after staging this report so the tracked-file guards
include it. Earlier interrupted gates are not counted as passing. Final gate
and independent review results are recorded after completion.

Final staged `cargo xtask ci`: **29 PASS**, no skipped steps. Workspace
4711 passed / 89 existing ignored; root integration 556 passed / 4 existing
ignored; root library 904 passed / 7 existing ignored. Temporary-directory
check passed. This report was tracked and staged before that complete run.
The earlier interrupted attempts are not counted as passing.
