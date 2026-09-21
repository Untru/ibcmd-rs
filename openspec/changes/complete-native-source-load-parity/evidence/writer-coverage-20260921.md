# What the native body writer covers, 21.09.2026

`src/compiler/bodies/form_native.rs` writes the records a native 8.3.27 form
body stores. Every function below is proved against strings read out of
`ibcmd_rs_uha_8327_parity2_20260920`, not against the writer's own output.

| record | wrapper | proved against |
|---|---|---|
| extended tooltip | `{12,…}` | one constant shape, 1 475 of 1 602 records |
| extended tooltip, visible variant | `{12,…}` | the other 49 |
| label decoration | `{12,…}` | two decorations of `ПомощникСозданияШаблонов` |
| field context menu | `{22,…8…}` | 14 records of one form |
| empty auto command bar | `{22,…9…}` | the list command bar of `ВыигранныеЛоты` |
| group, all kinds, with children | `{22,…}` | one childless record of six of the eight kinds |
| field (`LabelField` / `InputField`) | `{37,…}` | three label fields and one input field of `ВыигранныеЛоты` |
| standard-command button | `{31,…}` | two buttons of the same form |
| form attribute | `{9,…}` 16 members | 7 073 of 11 700 wrapper-9 records |
| form command | `{9,…}` 19 members | 3 936 of the same |
| table, around its six children | `{55,…}` | the offsets 344 table records agree on |
| search string, view status, search control | `{5,…}` | the three additions of one table |
| root layout, childless | `{50,…}` | two 48-member roots, forty members constant |

The table has no fixed member count -- its head grows with what it shows -- so
what is written is its structure: the command bar two members after the context
menu, the tooltip 26 members from the end, the three `{5,…}` additions 21, 19
and 17 from the end, and sixteen scalars closing the record. The head and the
scalar runs come from the caller until the slots that carry them are named.

Field kinds are not separate records: a picture, a spreadsheet document, an
HTML document, a radio-button group, an indicator and a formatted document are
the same 59-member `{37,…}` frame with another kind slot and another payload,
which `format_field_item` now writes.

## What is not written yet

- The root's child list: a root that carries items appends them after the
  command bar, and that count encoding is not measured.
- The payload tuple of each field kind other than the plain `{11,…}`, and the
  payload of each group kind.
- The root property bag and the DCS settings of a dynamic list.
- The scalar slots of each group payload, which decide which XML property each
  member carries.

## Discipline

A record is written only where the platform's own bytes say what it looks
like, and a property the writer cannot place is left to the blocker model to
refuse. Nothing here is wired into compilation yet: a half-written record would
load as a body the export then reads back differently, which is worse than an
honest refusal.

The evidence base for the rest is `F:\ibcmd\lab\form_bodies\Config_inflated` --
every one of the 12 515 ERP УХ form bodies, inflated -- together with the
censuses in this folder.
