# Standard picture references in MXL templates (#612)

The public fixture pin is `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
Both `corpus-spreadsheet-standard-picture` and `corpus-spreadsheet-facets`
carry the native picture record `{4,1,{0,<uuid>},"",-1,-1,0,0,""}`. The
affected template bodies use `v8ui:ShowData` and
`v8ui:AppearanceCheckIcon` in XML.

On `54b796f64da5daed3714acdf7810581fe0e58778`, the export gate with the
four new expectations failed on these two templates for XML 2.20 and 2.21.
The two templates wrote an empty picture. Removing the 16 known #612
refusals separately showed the writer treating the names as missing
`CommonPictures/*.xml` files. The corrected export and round-trip gates
both passed:

```sh
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::coverage_sets_hold_their_baseline --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_round_trip::coverage_sets_round_trip_through_ibcmd_rs_writers --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --lib mxl_standard_picture_prefers_a_declared_common_picture --no-default-features
```

## Standard and common picture references

In a temporary configuration with one copy of the synthetic MXL template
per vocabulary name, both native builds compiled all 221 names to
`{0,<uuid>}`. All stored UUIDs matched the existing standard-picture
vocabulary. With a common picture called `ShowData` declared alongside the
template, the same native import stored the common picture's UUID on both
builds. The regression test exercises the collision, an absent member of
a held source tree, a damaged common-picture file, and a standard reference
compiled without a configuration context.

The exporter passes the existing vocabulary lookup to the MXL decoder.
The writer looks up the common picture in its source context and uses the
standard vocabulary for an absent file. The dependency graph is unchanged.

## File infobase and CF replay

The locked `iter` binary was built from clean source
`c38a64192d5aee28a4e9b492a73f2c89f7f5fe4c` with
`cargo build --locked --profile iter`. SHA-256:
`8c91bcd59aba3b7cfe1b83d4ed58871c7d9fc773db076c3eb7e7730f65e5c9fa`.

| Native build | Pinned corpus | ShowData | AppearanceCheckIcon |
|---|---|---:|---:|
| 8.3.27.2214 | 8.3.27.2214 / XML 2.20 | 4 comparisons × 4 files | 4 comparisons × 2 files |
| 8.5.1.1529 | 8.5.1.1150 / XML 2.21 | 4 comparisons × 4 files | 4 comparisons × 2 files |

The executable returned zero for all 40 invocations across both tools and
both platforms. Every comparison used SHA-256 for the full selected file
set. `ConfigDumpInfo.xml` was omitted and paths were normalized to NFC.
For ShowData the selection was the entire isolated set. For
AppearanceCheckIcon it was the owning template XML and its body. The CF
and database exports used the pinned facets CF; the import and bootstrap
used the unchanged pinned template in the isolated set's configuration,
with its child list naming only that template.

For each platform and template, decode the pinned CF and verify its
manifest hash. In a new scratch directory use `LANG=ru_RU.UTF-8`,
the matching `BUILD`, separate native `--data` directories and these routes:

```sh
$OURS cf export "$PINNED_CF" "$RUN/cf" --platform="$BUILD"
$NATIVE infobase create --db-path="$RUN/loaded" --create-database --locale=ru_RU --load="$PINNED_CF" --apply --force --data="$RUN/data-loaded"
$NATIVE infobase config export --db-path="$RUN/loaded" "$RUN/loaded-native" --data="$RUN/data-loaded-export"
$OURS infobase config export --db-path="$RUN/loaded" --platform="$BUILD" "$RUN/loaded-ours"

$NATIVE infobase create --db-path="$RUN/import" --create-database --locale=ru_RU --data="$RUN/data-import"
$OURS infobase config import --db-path="$RUN/import" --platform="$BUILD" "$SRC"
$NATIVE infobase config apply --db-path="$RUN/import" --force --data="$RUN/data-apply"
$NATIVE infobase config export --db-path="$RUN/import" "$RUN/import-native" --data="$RUN/data-import-export"
$OURS cf bootstrap "$SRC" "$RUN/bootstrap.cf" --base-free --platform="$BUILD"
$OURS cf export "$RUN/bootstrap.cf" "$RUN/bootstrap-export" --platform="$BUILD"
```

Import used default verification. The comparisons cover the pinned CF
export, file database export against native from the same base,
native export after verified import and apply, and base-free CF bootstrap
and export. The fresh native exports from the pinned CF also matched
their selected pinned XML files.
