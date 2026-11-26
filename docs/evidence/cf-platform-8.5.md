# CF of platform 8.5: read and written offline (#354)

`ibcmd-rs cf export --platform 8.5.1`, `cf bootstrap --base-free --platform
8.5.1.1150` and `convert` between `xml-2.21` and `platform-8.5.1.1150` read and
write the `.cf` of a platform 8.5 configuration without the platform. The
writer is the base-free stage of the empty-database load
(`src/mssql/empty_stage.rs`, `src/metadata_model/`), whose 8.5 rows
8.5.1.1150 applied and exported identically for BSP 8.5 and ERP УХ 8.5
(`openspec/changes/complete-native-source-load-parity/evidence/empty-database-load-20260928.md`);
the container is the Format15 the base-free bootstrap writes
(`docs/evidence/cf-config-save.md`).

## The fixture

The one 8.5 configuration on record as a `.cf` is
`tests/fixtures/external/home_page/one_column_v85/input.cf`: the clean-room
base with two common forms on its home page, built from an XML 2.20 tree by
8.5.1.1529 at compatibility 8.3.27 and saved by it (`/DumpCfg`, Format16 behind
the platform's Format15 preamble). Beside it is the platform's
`Ext/HomePageWorkArea.xml`, dumped by 8.3.27.2214 after loading that file
(`_onecdec/make_home_page_fixtures.py`). No 2.21 dump of it by 8.5 is on
record, so its tree is ours (`cf export --platform 8.5.1.1150`), checked
against that file and against the platform's records below. The `.cfe` side
is covered by `tests/extension_v85.rs` (the extensions under
`external/v85_extension`, every file equal to the 8.5.1.1529 dump).

## What refused 8.5 before

| Where | What |
|---|---|
| `cf export --platform 8.5.1` | The Configuration row was not written (`Configuration.xml` missing): its `{76,...}` tuple holds `0,2,0,…,0,1,1` in members 61-68, and the 2.21 reader knew only the BSP combination `0,6,0,…,0,0,0`. Member 62 is the 8.5 code of `InterfaceCompatibilityMode` (6 for `Version8_5EnableTaxi`, 2 for `TaxiEnableVersion8_2`, as `metadata_model::root` already wrote it), not the theme. |
| `cf bootstrap --base-free --platform 8.5.1.1150` | Compiled, but with the 8.3.27 layouts for a 2.21 tree at compatibility 8.3.27 (the ERP УХ 8.5 rule): a form 8.5 left without a window opening mode came back as `DontBlock` with `Group` `Vertical`, and the report named `platform-8.3.27.1989`. |
| `convert xml → cf`, `platform-8.5.1.1150` | The bootstrap compiler needs the layout constants only the 8.3.27 profiles carry. |
| `convert cf ↔ xml` | `Ext/ClientApplicationInterface.xml` (no `version` attribute) failed dialect detection, on 8.3.27 as on 8.5. |
| `compatibility/matrix.json` | `convert-xml-221-to-cf-851-common-module`: unsupported. |

## What changed

- **Layout of the configuration, not of its compatibility.** Platform 8.5
  stores a configuration in one layout. The BSP 8.5 clone (`Version8_5_1`)
  is in the 8.5.1 layout, the ERP УХ 8.5 clone (`Version8_3_27`, carried over
  from 8.3.27) in the 8.3.27 one, and the fixture (`Version8_3_27`, built by
  8.5) in the 8.5.1 one: `{59,...}` form bodies, `{14,...}` form records and a
  `{76,...}` Configuration tuple. A 2.21 tree is now stored in the 8.5.1
  layout when its compatibility is 8.5 or later, or when one of its managed
  forms names no root `WindowOpeningMode` or no root `Group`: 8.5 prints
  both for every form stored the 8.3.27 way
  (`mssql_dump::form::xml_2_21_writer::upgrade_form_root`), so a form without
  one is held by the 8.5.1 layout only
  (`metadata_model::common::tree_stores_layout_8_5_1`). A tree with no form
  and an 8.3 compatibility mode keeps the 8.3.27 layout, as before.
- **The `{76,...}` tuple at compatibility 8.3.27.** Members 67 and 68 read
  `1,1` for `OpenDataInDialogs` and `DontUse`, the values 8.5 gives a
  configuration that names neither (the XML 2.20 the platform loaded names
  neither, and 8.5 prints them for a `{68,...}` tuple, ERP УХ). Which of the
  two members is which is not known; only the pairs `0,0` and `1,1` are read
  and written. Member 62 repeats the interface compatibility of member 38 in
  8.5 codes: `6` for `3` (БСП), `2` for `2` (the fixture).
- **`version`.** The format follows the compatibility, not the tuple: `216`
  below 8.5 (the fixture stores `{216,0,{80327,0}}` beside a `{76,...}`
  tuple), `217` at 8.5.1. The feature list is unchanged: the palette-colour
  feature when the configuration has palette colours, nothing else.
- **`cf bootstrap --base-free --platform <build>`** reports and uses the
  platform's own profile (`8.5.1`, `8.5.1.1529` → `platform-8.5.1.1150`, the
  profile with a storage layout).
- **`convert`**: a target profile without the bootstrap layouts (8.5) is
  compiled by the base-free stage (Format15, plan step
  `adapter:xml-to-cf-base-free`); a configuration's `Ext/` documents that are
  no metadata object pass through the canonical decode (a cross-profile
  route still refuses them).

## Round trip of `one_column_v85` (`tests/cf_platform_8_5.rs`)

`cf export --platform 8.5.1.1150 input.cf` → tree → `cf bootstrap --base-free
--platform 8.5.1.1150` → `built.cf` → `cf export` again:

- **File by file:** 11 of 11 files identical; `ConfigDumpInfo.xml` identical
  once `configVersion` is blanked.
- **`Ext/HomePageWorkArea.xml`** equals the platform's file but for the
  `version` attribute (`2.21` for `2.20`: the platform's file is the 8.3.27
  dump).
- **Record by record** against the platform's `input.cf`: the same 11 names;
  9 inflated records byte-identical (`root`, `version`, the language, both
  forms' records and bodies, the home page and the client interface). Two
  differ, both explained:
  - `versions`: the same 12 names, a fresh generation uuid each, by design.
  - the Configuration row: its footer, `{1,"",""},{-809843968}` saved by the
    platform, `{0,"",""}` written. The BSP 8.5 row's footer was
    `{1,"",""},{23}`, and 8.5.1.1150 applied and exported the BSP 8.5 load
    identically without it (the evidence above); the number is not derived
    from the tree.
- **`version`:** `{216,0,{80327,0}}`, byte-identical to the platform's: no
  feature listed, as the platform lists none for this configuration.

`tests/conversion_cli.rs` (`xml_221_cf_851_xml_roundtrip_is_offline`) adds a
common module in 8.5.1.1529's own 2.21 spelling
(`v85_extension/test_extension`) to the same tree and converts it to a
`platform-8.5.1.1150` CF and back: every file equal.

## Open, needs the platform

- The platform's `config load` (or `/LoadCfg`) of a built 8.5 `.cf` and its
  dump: no platform runs here, so acceptance by 8.5.1.1150 itself is not
  proven for the container. The rows are those 8.5.1.1150 applied in the
  empty-database loads above, and every record but the footer and the
  generations equals what 8.5.1.1529 saved.
- A 2.21 dump of the fixture by 8.5, to compare our tree with the platform's
  file by file (only `Ext/HomePageWorkArea.xml` is on record, in 2.20).
- Which of tuple members 67 and 68 holds the windows open variant and which
  the interface migration mode, and the codes of the other values of the
  8.5 enumerations.
- A 2.21 tree at an 8.3 compatibility mode with no managed form: which
  layout a fresh 8.5 save gives it.
