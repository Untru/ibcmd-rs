# MXL group/header color namespaces: issue #611

## Inputs and rule

Base: `0c988ca869c6a718f1ab7111e29e370f6fe76464`.
Pinned public 1c-fixtures: `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
The existing generated `CovObservedSheetGroupColours` variant is
`platform_proven` and reproduces the issue; no application object is used.
No corpus inputs or gitlink changed. The existing combined sets already
include the variant, and the coverage gate verified their source digests;
therefore no combined rebuild was needed.

Document-level groupsBackColor/groupsColor/headersBackColor/headersColor
use a local `d2p1` namespace declaration. Cell format colors retain `d3p1`.
Web and Windows namespaces are distinct. Style references, RGB strings and
existing text escaping remain unchanged. The decoder and physical slots did
not change; this is a writer bug fix, not a new OpenSpec capability.

## Failing first

`cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features -- --nocapture`
first returned 101: 5 passed, 1 failed. The new complete-file comparison
failed at `8.3.27.2214`: candidate SHA-256
`4fa63f6673f8cb7a89c9413d5d6a48d9e1f9f52be47b51ec8b628a463c820d50`,
native `cb610f33fd7b871899e9fe6b67792cff4dd28660b4e738bc86b4741a30c03d29`.
The coverage report also marked the same generated variant different on
both XML versions. All pre-existing baselines held.

## Fresh native macOS measurement

macOS 26.7.1 arm64; native ibcmd 8.3.27.2214, SHA-256
`1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`.
Both sides used `LANG=ru_RU.UTF-8`. Every native command had its own data
directory. All infobases were newly created disposable synthetic copies.

Recipe: subset the pinned variant's native configuration to its one
CommonTemplate using `tools/coverage/native_round_trip.py:subset` in
1c-fixtures. Keep the original as `existing`. For `all-web`, also set
headersBackColor to web:FireBrick and headersColor to web:White. For
`windows`, set all four roles to windows:ButtonShadow with an explicit
input namespace declaration. No source tree from a business application is
involved.

```text
ibcmd infobase create --db-path=<db> --create-database --locale=ru_RU --import=<tree> --apply --force --data=<unique-data>
ibcmd infobase config export --db-path=<db> <native> --data=<unique-data>
ibcmd infobase config save --db-path=<db> <configuration.cf> --data=<unique-data>
ibcmd-rs cf export <configuration.cf> <fresh-cf-output> --source-version 2.20
ibcmd-rs infobase config export --db-path=<db> --platform=8.3.27 <fresh-db-output>
```

Before the fix, all six candidate commands returned 0 but each tree differed
in exactly the Template.xml file. The native measurements established the
same local d2p1 rule for all four roles and for both color namespaces.
After the fix, all six commands returned 0 and all **30/30 files** matched
raw native bytes, including ConfigDumpInfo.xml, with no normalization or
exclusions. Whole database SHA-256 was unchanged after every candidate
command, both before and after the fix.

| case | CF / database files equal | native and candidate tree SHA-256 |
| --- | --- | --- |
| existing | 5/5 + 5/5 | `a174237c0df1aad5892d8179ac90c2da305a39b3c364720902ac95e684b8472c` |
| all-web | 5/5 + 5/5 | `8a882541a2fd338a998e9dec2fb7dad435d2db588e393f8d7f8818a8efa34242` |
| windows | 5/5 + 5/5 | `59a32c3b507db5027cc2b520e41bc0ada2ef463c5779a48acc7a53bdcd5d8606` |

Tree hashes encode sorted UTF-8 path + NUL + lowercase file SHA-256 + NUL +
decimal byte size + LF. Full native trees and databases stay outside Git.

## Portable regression evidence

- Rust 1.95.0; no new dependencies.
- `cargo test --locked -p ibcmd-moxel`: 39 passed.
- `cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features -- --nocapture`: 6 passed after the fix, including complete native template comparisons for 8.3.27.2214 / XML 2.20 and 8.5.1.1150 / XML 2.21.
- 11 variants became newly equal. Only those additions entered converter-owned baselines; 5251 already newly-equal variants outside this issue were not added. Existing baseline line endings were preserved.
- `cargo xtask check`: 5 steps passed, including the coverage gate with the new baselines.
- `cargo test --locked -p ibcmd-rs --test integration coverage_storage_records::clean_room_corpus_stored_row_route_holds_its_baseline --no-default-features -- --nocapture`: 1 passed; both native 8.3.27 and 8.5.1 stored-record sets produce the complete byte-equal target template. Those two paths are now protected by storage baselines.

The stored-record check uses the shared database XML-export pipeline and
native CF records with PATH empty; it is not a live SQL Server measurement.
Fresh native runtime measurements used 8.3.27.2214. 8.5 checks use the
immutable native 8.5.1.1150/1529 corpus, not a freshly installed 8.5 runtime.
Other known coverage differences/refusals remain outside #611.
