# Tasks

Every line here is measured against the ERP УХ corpus -- 12 507 forms, their
native 8.3.27.2214 export and the inflated bodies of the same database -- and
the number quoted is the share of stored records a candidate writer reproduces
byte for byte from the source alone. A writer refuses what it has not measured
rather than defaulting it.

## Done

- [x] **The body frame.** Ten members in all 12 507 bodies: root record, module
      text, attributes, parameters, commands, two appearance sections, `0`, `0`.
      *(body-frame-20260921.md)*
- [x] **The root record's head** -- 18 members. 12 469 / 12 469, refusing the 19
      forms that name a `<SettingsStorage>`. *(root-head-and-bag-20260921.md)*
- [x] **The root record's property bag** -- its shape: a count, that many
      `(key, value)` pairs, then exactly four members. Holds in all 12 488
      records that split. The keys belong to the form extension of the main
      attribute, one numbering per class.
- [x] **The root record's tail** -- 21 members after the two empty strings and
      the optional navigator. 12 410 / 12 410, refusing the 78 forms that carry
      a `<MobileDeviceCommandBarContent>`. *(root-record-20260921.md)*
- [x] **Event bindings**, of the form and of every item. 63 161 / 63 161.
      *(event-bindings-20260921.md)*
- [x] **Form attributes** -- the `{9,…}` record. 94 031 / 94 031 of the fixed
      part, with six members the caller supplies because they name
      configuration objects. *(form-attributes-20260921.md)*
- [x] **Form parameters** -- 24 863 / 24 863.
      *(form-parameters-and-commands-20260921.md)*
- [x] **Form commands** -- 60 983 / 61 228 (99.60%).
      *(form-parameters-and-commands-20260921.md)*
- [x] **The `{22,…}` container record**, across all nine kinds: head
      9 357 / 9 357, tail 9 354 / 9 357. *(group-record-20260921.md)*
- [x] **The `{37,…}` field record**, across every field kind:
      122 630 / 122 767 (99.89%). *(field-record-20260921.md)*
- [x] **The `{31,…}` button record**: 76 913 / 77 127 (99.72%), the rest
      differing only in a member the element does not carry.
      *(button-record-20260921.md)*
- [x] **The `{12,…}` decoration record**, one layout for tooltips, labels and
      pictures: 464 526 / 464 539 (100.00%).
      *(decoration-record-20260921.md)*
- [x] **The `{55,…}` table record's tail** -- the 37 members after its
      columns: 6 890 / 6 903 (99.81%). *(table-tail-20260921.md)*
- [x] **The `{55,…}` table record's head** -- the 54 members before its bag:
      6 900 / 6 903 (99.96%). With this every item record of a form body is
      read. *(table-head-20260921.md)*
- [x] **Colours** and **fonts** an item can carry.
- [x] **The item payloads, every member of each.** The partition test closed
      the usual group (24 of 29 members over 61 256 records), the auto command
      bar, the button group, the command bar, the pages group, the popup, the
      column group, the page, the check box, the radio button, the spreadsheet
      document and the picture, and all of them are wired to the XML.
      *(container-and-field-payloads-20260921.md)*

## Open

- [x] **The root record's property bag values, per main attribute class.** The
      shape is read; what each key holds is not. The table's own bag is read
      -- see table-property-bag-20260921.md -- and the root's is the same kind
      of store. A dynamic-list form writes key 1, a
      document form 2, 3, 4 and 24, a catalog form 0 and 24, a report form 5 to
      22 with 27 and 29.
- [x] **The two appearance sections** of the frame.
- [ ] **The settings blob** is *not* in the source. Two spellings account for
      11 842 of 12 507 bodies and nothing in the XML separates them, so the
      writer picks the canonical empty one and the round trip closes on the
      second export, not on the original database.
- [x] **A path from `Form.xml` to a body, and a way to measure it.**
      `compile_native_form_body` writes the frame, the root record and the
      auto command bar; `ibcmd-rs audit-native-form-writer` compares what it
      writes to what the platform stored, for every form of a tree. Load
      parity is measurable end to end for the first time.
      *(native-writer-wired-20260921.md)*
- [x] **The attributes section**, with every attribute's type pattern resolved
      against the configuration. 41 whole bodies now rebuild byte for byte.
      *(first-bodies-written-20260921.md)*
- [x] **Call the item writers.** `format_native_child_item` dispatches on the
      item's tag -- containers, fields, buttons and decorations -- and the
      parameters and commands sections are written too. `child items` has left
      the refusal list entirely: 126 forms now reach comparison, up from 83.
- [x] **Resolve a button's `<CommandName>`.** Always `{<target>,<uuid>}`: the
      target is 0 for a form standard command and the target item's own id for
      an item standard command -- true in all 15 692 standard-command buttons
      -- and the command's own id for `Form.Command.X`. The uuid is fixed per
      scope and name, where the scope is the form's main attribute class or the
      target item's tag plus whether it is bound to a dynamic list; 146 of 147
      form keys and 156 of 158 item keys map to one uuid, and the rest are
      refused.
- [x] **The `{55,…}` table record.** Head, keyed property bag, events, context
      menu, command bar, columns and tail, all wired. The bag's key set is
      decided by the type of the attribute the table binds to, and every
      non-constant key carries one XML property, pure over the 10 738 records
      that split. *(table-property-bag-20260921.md)*
- [x] **A dynamic list's settings** (3 204 forms), the largest refusal left. The bag is
      transcribed from the source and the field map is synthetic -- its ids are not
      in the source and the export does not read them. *(findings/rt-dynamic-list.md)*
- [x] **The `{5,…}` addition records** a table's `<SearchStringAddition>`,
      `<ViewStatusAddition>` and `<SearchControlAddition>` carry -- 24 members
      each, in tail slots 16, 18 and 20 (1 704 forms).
- [x] **The navigator** is not a source property at all: it travels with the
      settings composer spelling, which is not in the source either. Of the
      11 842 forms carrying one of the two canonical blobs, 11 821 agree --
      99.8%. Both come from what the platform wrote when the form was last
      saved, so the writer pairs the empty settings with no navigator and the
      round trip closes on the second export.
      *(navigator-and-generation-20260921.md)*
- [x] **The form's own `<Enabled>`**, member 15 of the root head, which the
      parser does not read.
- [x] **The parser's own gaps**, which hold up 2 620 forms before the writer
      ever sees them: `<ExcludedCommand>` spellings (1 455), conditional
      appearance (569), list settings (401), DCS children (195).
- [ ] **Close the round trip**: export → load into an empty database → export,
      byte-identical to the second export. This, not the body-to-body audit, is
      the criterion: the navigator and the command bar's functional-options
      block are not in the source at all, so `different` can never reach zero.
      *(navigator-is-not-in-the-source-20260921.md)*

## The form round trip, 2026-09-23

`F:\ibcmd\lab\tools\rt_compare.sh` compiles every `Form.xml` with the native
writer, exports the database with those bodies in place of the stored ones and
diffs the export against the native tree. A form passes when it compiles and its
`Form.xml` comes back unchanged:

| | forms | compiled | unchanged |
|---|---|---|---|
| BSP | 1 108 | 1 108 | 1 108 (100 %) |
| ERP УХ | 13 044 | 13 044 | 13 044 (100 %) |

Every export file is unchanged on both corpora. What is not in the source and
is therefore neutral to the round trip: the navigator, the dynamic-list field
map ids, a chart's legend layout, a constants set's always-used flags (a delta
the target database decides -- `IBCMD_RS_ALWAYS_USED_CONSTANTS`), and the empty
settings blob. The loader prefers the native writer for a new or changed form
(`IBCMD_RS_NATIVE_FORM_WRITER=always` for every form); the database cycle --
the load of every other object kind, then export -- is the step that remains.

## The database cycle, measured without writing SQL (2026-09-23)

`F:\ibcmd\lab\tools\vcycle.sh <bsp|uha> <run>`: `mssql-audit-source-parity`
writes every row a load would stage (`IBCMD_RS_WRITE_STAGED_ROWS_DIR`),
`mssql-dump-config` exports the database with those rows in place of its own
(`IBCMD_RS_ROW_OVERRIDE_DIR`), and `source-diff` compares the export with the
native tree. The file diff is the verdict; a row whose plain text differs but
exports identically (forms, layout-only differences) is not a failure.

First БСП result: 12 082 of 12 198 files unchanged (99.05 %). Fixed on the way,
each measured over both corpora: help rows' per-class suffix, the configuration
asset owner, constant/defined-type string and date qualifiers, picture
transparency, detailed job schedules, exchange-plan AutoRecord and trailer,
bodyless common modules, the 64 MiB template ceiling, help/picture layout.

Each done in its own branch and merged into `fix/8-3-27-parity`:

- [x] DCS templates compile base-free and round-trip (`feat/dcs-template-writer`)
- [x] Role rights compile base-free and round-trip (`feat/role-rights-writer`)
- [x] Command interface, home page, client application interface, standalone
      content compile base-free and round-trip (`feat/interface-assets-writer`)
- [x] Spreadsheet templates stored as the platform stores them
      (`feat/mxl-template-writer`; MOXCEL framing -- native ibcmd refused the
      earlier compact bodies although our exporter read them)
- [x] Help pages and HTML templates stored with the platform's link spellings
      and CRLF (`feat/help-links-writer`)
- [x] БСП virtual cycle: 12 198 / 12 198 files, 0 prepare failures
- [x] БСП real cycle on the disposable clone (2026-09-23, `F:\ibcmd\lab\realcycle\bsp_r3`):
      stage 9 514 rows in 3 minutes, publish (dropping the dynamic-update
      leftovers), export with ibcmd-rs and with native ibcmd 8.3.27.2214 --
      native accepts everything, both exports reproduce 12 197 / 12 198 files
      and agree with each other; the last one is `ConfigDumpInfo.xml`, whose
      configVersion values are the new generation the load wrote
- [x] ERP УХ virtual cycle (2026-09-23, run v8, commit ce082eac): 140 709 /
      140 709 files, 0 prepare failures. On the way: style bodies in the
      8.3.27 layout, all flowchart item shapes, StdPicture tables for command
      pictures, WSReference Format15 containers, additional indexes compiled,
      graphical schema templates through the flowchart grammar, entities and
      untrimmed free text in predefined data and flowcharts, command picture
      transparency pixels
- [ ] ERP УХ real cycle on a disposable УХ clone (needs Pavel's go-ahead)
- [x] Platform 8.5 (2.21) virtual cycles on the 8.5 clones (branch
      `feat/8-5-source-export`): БСП 12 337 / 12 337, ERP УХ 140 709 /
      140 709 files, 0 prepare failures
- [x] 8.5 real cycle pre-flight (2026-09-24, read-only):
      `mssql-stage-source-objects --script-only` writes every batch of both
      8.5 clones -- БСП 10 batches, 206.7 MB, 9 618 rows; ERP УХ 112 batches,
      3.36 GB, 116 651 rows -- byte-identical to the virtual cycles' staged
      rows, every Config row the scripts require present. Steps, expected
      counts, disk and risks: `F:\ibcmd\lab\v85\findings\STATUS.md`,
      "8.5 real cycle -- ready to launch"
- [x] 8.5 form gaps of the pre-flight (2026-09-24): the loader spells a form
      body's leaves the 8.5 way (palette namespace in embedded settings
      documents, 64-character base64 lines with CR CR LF, CR LF in strings)
      and writes a report form's state (default report form URNs, report
      name, variant `Основной`) -- БСП 8.5 plain-identical rows 8 487 ->
      9 226 of 9 619, the 9 report forms' root bags equal the stored ones;
      virtual cycles stay at 100 %
- [ ] 8.5 real cycle on disposable 8.5 clones (needs Pavel's go-ahead)

Metadata descriptor rows (3 902 in БСП) are still patched onto the target's
existing rows; a load into an empty database needs a descriptor compiler per
metadata class and is not part of this cycle.

## The tool that turned out to matter

`partition-table-head.py` asks, for one member, **which property maps each of
its spellings and its absence to exactly one stored value over every record** --
not which property explains the records that differ. The witness search is
misled by any property that is merely common, and it sent one reading from 57%
down to 31%. The partition search named fifteen members of the table head in a
single pass, four of them with an absent value that is not 0 and that a witness
search could never have shown. Reach for it first on any member that resists.

## What the measurements keep turning up

Five properties are written **twice, under two different codings**:
`<VerticalScroll>` and `<Group>` in the root tail, `<CurrentRowUse>` in a
command, and `<Type>` and `<LocationInCommandBar>` in a button. In every case
the second reading is the finer one -- a command-bar hyperlink is a plain
command-bar button at member 4 and itself at member 46. Reading any of them
once costs thousands of records, so when a candidate stalls just short, look
for the property that is written twice before looking for a new one.
