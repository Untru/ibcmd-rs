# The revision-3 form command bar, 20260826

Base `8cc12dc`. Corpus: all eight stand configurations
(`$D = /Users/untru/Documents/ChatGPT/ibcmd-stand`), gated against
`$D/baselines/8cc12dc/`.

## The defect this closes

`uh-form-body-container-revision-20260826.md` opened the 102 ERP УХ 3.2.12.6
form bodies that declare container revision `3` and then measured what the
reader made of them. Split by container revision and by whether the platform's
own `Form.xml` gives the form a populated `<AutoCommandBar>`:

| container revision | populated command bar | exact | differing | missing |
|---:|---|---:|---:|---:|
| 4 | no | 4 375 | 559 | 14 |
| 4 | **yes** | **7 080** | 822 | 45 |
| 3 | no | 34 | 27 | 0 |
| 3 | **yes** | **0** | **41** | 0 |

Zero of forty-one is not diffuse rendering debt. Under revision 4 a populated
command bar comes out right 7 080 times; under revision 3 it never did.

## Census of the revision-3 item records

Every item record of all 102 revision-3 layouts, anchored on the class uuid the
platform writes immediately before each record (dumped by the probe of
`8d2bd9d`, removed again at the end of this pass):

| class | leading member | members | records | reader at `8cc12dc` |
|---|---:|---|---:|---|
| `Button` | `29` | 49 | 164 | **none** |
| `Decoration` | `11` | 35 | 101 | `11` → `12`, pad 1 |
| `Field` | `34` | 56 / 57 | 418 / 12 | `34` → `37`, pad 3 |
| `Group` | `22` | 30..44 even | 146 | canonical |
| `Table` | `54` | 100..154 | 55 | `54` → `55`, pad 1 |

Facts this establishes:

* Exactly one item revision in these bodies had no reader: `Button` `29` at 49
  members. Every one of the 164 button records declares it; there is no second
  length, and not one canonical `31` or short `30` among them.
* The group class is *not* short here. Besides the 146 class-anchored group
  records, the layouts hold 857 unanchored `{22,…}` blocks at 29 members and
  more at 31, 33, 35, … — the canonical `29 + 2k` shape of the nested
  `AutoCommandBar`/`ContextMenu`, which the platform writes as a direct member
  of its owner rather than through a counted class-anchored pair. Nothing about
  the container revision shortens a group record.

## Where the buttons were lost

`form_child_item_tag` returned `None` for leading member `29`, so
`parse_form_child_item_with_metadata_owners` returned `None`, so
`parse_form_child_item_pairs` could never make `items.len() == count` at the
one position that was real. Both of its call sites —
`extract_form_child_items` and `parse_form_auto_command_bar_fields` — turn that
into `.unwrap_or_default()`, an empty item vector. The form is still written,
still passes every writer preflight, and the loss carries no diagnostic:

```
native:  <AutoCommandBar name="ФормаКоманднаяПанель" id="-1">
             <Button name="ФормаОК" id="14">…Form.Command.ОК…</Button>
             <Button name="ФормаЗакрыть" id="13">…Form.StandardCommand.Close…</Button>
         </AutoCommandBar>
ours:    <AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>
```

## `Button` `29` is `31` minus three trailing scalars

Coarse slot shape (`n` scalar, `s` quoted string, `b:k` block of `k` members),
the one shape all 164 records take, against the 16 876 canonical `31`/52 records
of the dumped `ws`/`wms`/`mdm`/`sslbase`/`ssl`/`do` layouts:

```
29/49  n b:2 n n n s b:2 n b:2 b:1 n×9 b:3 b:3 b:3 b:5 b:3 n b:9 n b:1 s n n n b:33 b:1 n×15
31/52  n b:2 n n n s b:3 n b:2 b:1 n×9 b:3 b:3 b:3 b:5 b:3 n b:9 n b:1 s n n n b:34 b:1 n×18
```

Member for member over all 49 members, with two differences that are not shape
differences: slot 6 is the localized-title tuple, whose inner length follows the
number of languages, and slot 32 is the button's own `ExtendedTooltip`, itself
the decorations' short revision `11` at 33 members against the canonical `12` at
34 — which `normalize_form_item_record_revision` already reads. The trailing
scalar run falls 18 → 15.

Independently, the value vocabulary of every scalar slot of `29`/49 is a subset
of the same slot's vocabulary in `31`/52, with two exceptions: slot 0, which is
the revision itself, and slot 16, where ERP УХ writes a width of `60` that the
smaller corpora never reach. Slots 49, 50 and 51 exist only under `31`.

One thing the earlier census did not predict: `field_count - wrapper` is **20**
here, not the 21 that `30`/51 and `31`/52 share. Revision `30` added two
trailing members while the leading member advanced by one. The leading member
names the revision; only the measured arity says which shape it is, which is why
the guard pins the single length the corpus writes and refuses a `29` record of
any other length rather than padding it on the strength of a formula.

Collision pre-flight, over every braced block of every dumped layout — the 102
revision-3 `uh` layouts, 2 800 `ws`/`wms`/`mdm`/`sslbase`/`ssl`/`do` layouts,
5 201 `ut` layouts and 12 895 revision-4 `uh` layouts: no `{29,…}` block of 49
members is anything but a class-anchored button. The other `{29,…}` blocks are
the `UsualGroup` extended-options bag at 29 members (which
`form_property_bag_canonical_revision` already documents) and one- and
two-member value tuples, neither of which the arity guard can reach.

## `LocationInCommandBar` moved coordinate between the two revisions

Admitting the record is not the whole of it. The canonical revision keeps the
button's `<LocationInCommandBar>` in slot 49 — one of the three members revision
`29` does not carry, so normalizing pads it as absent and the element goes
unwritten. The platform writes it anyway on 13 of the 164 records.

Revision `29` keeps the property in slot 15, the three-valued predecessor the
canonical revision carries alongside its own slot 49. Over all 27 703 `31`/52
button records of the 5 201 UT 11.5.27.75 layouts the two slots are in exact
correspondence, with no other pair occurring:

| slot 15 | slot 49 | platform | records |
|---:|---:|---|---:|
| `2` | `0` | *(no element)* | 23 033 |
| `0` | `1` | `InAdditionalSubmenu` | 2 991 |
| `1` | `2` | `InCommandBar` | 457 |
| `1` | `3` | `InCommandBarAndInAdditionalSubmenu` | 1 222 |

Slot 15 names the property outright except inside its own `1`, where the
canonical revision refines "in the command bar" into "with" and "without" the
additional submenu — the one distinction revision `29` has no member to spell.
All 164 revision-`29` records agree with that reading against the platform's own
XML: `InAdditionalSubmenu` on the six whose slot 15 is `0`,
`InCommandBarAndInAdditionalSubmenu` on the seven whose slot 15 is `1`, and no
element on the other 151.

The fallback can only fire on a revision-`29` record: it keys on the canonical
member being the absent placeholder, and no other admitted revision pads slot
49 (revision `30` pads one member, at slot 51).

## The option block of a check box declares its own revision too

Eleven of the same revision-3 forms drop a `<CheckBoxType>Auto</CheckBoxType>`
the platform writes explicitly — a default mistaken for an absence, doctrine
point 6. `FormCheckBoxFieldSchema::from_raw_layout` demanded exactly
`("11", 13)` for the option block at field slot 39 and read the code from slot
12. Under container revision 3 that block is `("10", 12)`, so the schema refused
it whole.

Every field record's option block is one `(leading member, length)` per
discriminator, per revision:

| discriminator | revision 3 | revision 4 |
|---:|---|---|
| 1 `LabelField` | `11`/20 | `11`/20 |
| 2 `InputField` | `32`/62 | `36`/66 |
| 3 `CheckBoxField` | **`10`/12** | `11`/13 |
| 5 `RadioButtonField` | `8`/12 | `8`/12 |
| 6 `SpreadSheetDocumentField` | `12`/31 | `13`/32 |
| 15 | `3`/13 | `3`/13 |

`10`/12 is `11`/13 minus its final member: slots 0..11 agree in kind and in
value distribution, `len - lead` is 2 for both, and `11` appends slot 12.

The code revision `10` reads is slot 4, the three-valued predecessor revision
`11` still mirrors. On all 887 revision-`11` blocks of the БСП base tree slot 4
equals slot 12 wherever slot 12 is `0`/`1`/`2`, and reads `0` on the eight where
slot 12 is `3` — `Switcher`, the ordinal revision `11` added. On all 18
revision-`10` blocks the stand carries, the platform's own `<CheckBoxType>` is
`Auto` on the sixteen whose slot 4 is `0`, `CheckBox` on the one that reads `1`
and `Tumbler` on the one that reads `2`, with no counter-example. Slot 4 stays
unread under revision `11`, where slot 12 is authoritative and already proven
byte-for-byte across the corpus.

Collision pre-flight: across the 2 800 small-corpus layouts, the 5 201 `ut`
layouts and the 12 895 revision-4 `uh` layouts, every discriminator-3 option
block declares `11`/13 — 2 417, 4 751 and 9 012 of them respectively — and not
one declares `10`/12. The short block occurs only under container revision 3.

## Result

Exact-set difference against `$D/baselines/8cc12dc/<key>.parity.json`, sets not
counters:

| key | base exact | now | gained | broken | extra |
|---|---:|---:|---:|---:|---:|
| `ws` | 29 | 29 | 0 | 0 | 0 |
| `wms` | 226 | 226 | 0 | 0 | 0 |
| `mdm` | 160 | 160 | 0 | 0 | 0 |
| `sslbase` | 9 614 | 9 614 | 0 | 0 | 0 |
| `ssl` | 12 692 | 12 692 | 0 | 0 | 0 |
| `ut` | 50 896 | 50 896 | 0 | 0 | 0 |
| `do` | 25 201 | 25 201 | 0 | 0 | 0 |
| `uh` | **138 467** | **138 494** | **+27** | 0 | 0 |

`uh` 98,6155 % → 98,6347 %. All 27 gained files are revision-3 forms. Attributed
by the record revisions each carries: 23 carry only `Button` `29` records, 2
carry only a `10`/12 check-box option block and no `29` record at all, and 2
carry both.

Inside the revision-3 population, by populated command bar:

```
                          before   after
revision 3, bar populated   0/41    21/41
revision 3, bar empty      34/61    40/61
```

## Still open on these forms

The 20 revision-3 forms with a populated command bar that remain `differing`
were re-diffed against native: 145 changed lines in all, seven of the twenty
down to a single line. The residue is no longer the command bar — not one
changed line touches
`AutoCommandBar`, `Button`, `CommandName`, `Type`, `DefaultButton`,
`ExtendedTooltip`, `ChildItems`, `CheckBoxType` or `LocationInCommandBar` any
more. By tag, counting files rather than lines:

| tag touched | files (of 20) |
|---|---:|
| `Representation` | 14 |
| `Mask` | 4 |
| `ChoiceList` (`xr:Item`/`xr:CheckState`/`xr:Value`) | 2 |
| `VerticalSpacing`, `ViewModeApplicationOnSetReportResult` | 2 each |
| `HorizontalStretch`, `PagesRepresentation`, `ItemTitleHeight`, `ShowTitle`, `ChildItemsWidth`, `HorizontalAlign`, `DataPath`, … | 1 each |

The dominant one is a `UsualGroup`'s `<Representation>None</Representation>`,
read from the group's extended-options bag — the short revision `28`/28 against
the canonical `29`/29 that `form_property_bag_canonical_revision` documents and
does not admit. That is a sibling task's ground, and it is already fixed on
`worktree-agent-a6877a6d7c5c0a5bd` (`730b89f`, not an ancestor of this base).

### The `InputField` option block never reaches its normalizer

Measured, not fixed, and the sharpest of the remaining leads on these forms.
`form_input_field_extended_options` runs the option block through
`normalize_form_property_bag_revision`, so the `InputField` short revision
`32`/62 is read there; `FormFieldSchema::from_raw_layout`
(`form_body.rs`, the `field_schema_and_options` binding) is handed the **raw**
block and demands `(66, "36")` outright. Under container revision 3 that gate
therefore refuses, `input_field_options` comes out `false`, and every
option-driven `InputField` property — `Mask`, `MinValue`, `MaxValue`,
`ListChoiceMode`, `QuickChoice` and the rest of the 50-slot table — goes
unread.

On this bucket it shows up as `<Mask>` alone, on four files — and on three of
them it is now the *entire* remaining diff, one line each:
`Catalogs/МакетыПенсионныхДел/Forms/ВводСНИЛС`
(`<Mask>999-999-999 99</Mask>`, present verbatim at option slot 18 of its
`32`/62 block) and
`Catalogs/ЗаявлениеОНазначенииПенсии/Forms/`{`Дети`,`ИнвалидыПожилые`}; the
fourth, `CommonForms/ВводРеквизитовОПАлко`, needs a `Representation` too. The
blast radius is far wider than this bucket, and that is the point: the short
`InputField` bag is not tied to the container revision at all. The
class-anchored census finds `32`/62 on 203 field records of the 102 revision-3
layouts and on 1 542 more of the 12 895 revision-4 `uh` layouts, against 67 192
canonical `36`/66 — so handing `FormFieldSchema` the normalized block reaches
roughly 1 750 `uh` records and needs its own gated pass rather than a ride on
this one. The reverted `ce175ea` is a warning about exactly this neighbourhood.

Left deliberately unread here, as a typed absence rather than a guess: the
`SpreadSheetDocumentField` option block's short revision `12`/31 against the
canonical `13`/32. It occurs on three records in three files
(`Documents/ВыгрузкаРегламентированныхОтчетов/Forms/ФормаСообщенийОбОшибках`,
`Reports/БухгалтерскаяОтчетностьВБанк/Forms/ДетальныйПереченьОпераций`,
`Reports/ОтчетПоНекорректнымКонтрагентам/Forms/Форма`), none of which has a
populated command bar, so all three sit outside this pass's bucket and none has
had its own byte-level pass.

## Reproducing

The probe that produced the layout dumps is commit `8d2bd9d`
(`IBCMD_FORM_LAYOUT_DUMP`, `IBCMD_FORM_LAYOUT_DUMP_REV`), reverted by `881cdf4`.
It was verified inert before anything else: a full `uh` export at base `8cc12dc`
with the dump enabled reproduces `$D/baselines/8cc12dc/uh.parity.json`'s exact
set byte for byte (138 467, `BROKEN=0`, `extra=0`). Every gate figure in the
table above was then re-measured on the probe-free binary, after the revert.

`cargo test --lib` 2 334 passed / 33 failed, the failing names identical to
`$D/baselines/8cc12dc/fail-base.txt`; three of the passing ones are this pass's
own real-byte regression tests, each a negative control that fails without its
own fix. `cargo test -p ibcmd-schema` 108/108. `bundled9.sh` 9/9.
