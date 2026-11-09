# External report attribute export (#554)

The input is the generated `8.3.27.2342/external/{erf-attributes,combined-erf}`
corpus pinned at 1c-fixtures `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
The oracle is the stored XML from the exact platform build in each manifest.
The corpus contains 100 platform-proven individual reports and one
platform-proven combined report. One generated input was refused by the
platform and has no binary fixture.

Add all platform-proven cases to the coverage expectations and run:

```sh
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::coverage_sets_hold_their_baseline --no-default-features -- --nocapture
```

On `54b796f64da5daed3714acdf7810581fe0e58778` with the expectations alone,
35 individual reports and the combined report were refused with
`failed to move …/Reports`. The strict report attribute reader had refused
the type pattern before writing the root XML. The corrected reader keeps
well-formed unnameable type IDs and understands the Null and SettingsComposer
leaves in the identified external report. The configuration reader uses its
existing type rules.

The corrected corpus run exported all 101 platform-proven reports with
byte-identical XML and preserved the other expectations.

## Fresh native spot checks

The installed 8.3.27.2214 binary (SHA-256
`1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`)
re-exported five inputs to the same bytes as the pinned 8.3.27.2342 XML.
The inputs cover Null, SettingsComposer, an unnamed TypeId, the template and
the combined report. For each input, decode its manifest's `.erf.b64` into a
scratch file and verify the decoded SHA-256, then run:

```sh
mkdir -p "$scratch/tree"
LANG=ru_RU.UTF-8 ibcmd infobase create --db-path="$scratch/ib" --create-database --locale=ru_RU --data="$scratch/create"
LANG=ru_RU.UTF-8 ibcmd config export --db-path="$scratch/ib" --file="$input" --data="$scratch/export-$case" "$scratch/tree/$case"
```

Compare the exported file set and its SHA-256 values with the case manifest.
These spot checks supplement the exact-build corpus oracle.

For a composition probe, start from the pinned synthetic report with one
unnamed TypeId and add `<v8:Type>xs:boolean</v8:Type>` and
`<v8:TypeSet>cfg:AnyIBRef</v8:TypeSet>` to its attribute's `Type` block.
Compile that edited XML with native
`config import --db-path="$scratch/ib" --out="$scratch/mixed.erf" --data="$scratch/import"`
and export the compiled file with both tools. Native XML places the named
type first, the type set second and the unnamed TypeId last. The regression
test also checks refusal of extra members, nil UUIDs and opaque trailing data.

The corrected export of the composition probe matched the whole native XML byte for byte.
