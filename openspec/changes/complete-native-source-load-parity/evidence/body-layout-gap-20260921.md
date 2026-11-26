# The load side does not target the layout the platform stores, 21.09.2026

The blocker census says which forms the source compiler refuses. This note
records *why* closing those refusals one by one would not by itself reach
parity, because the two sides are not aiming at the same artefact.

## What the platform stores

Four form bodies read straight out of `ibcmd_rs_uha_8327_parity2_20260920`
(`mssql-dump-config --inflate --file-name <row>`), from four different
families:

| form | record wrappers, by count |
|---|---|
| `Catalogs/ШаблонЦепочкиПлатежей/…/ПомощникСозданияШаблонов` | 12×47, 22×41, 37×21, 31×9, 9×7, 5×6, 55×2 |
| `BusinessProcesses/Задание/Forms/ДействиеПроверить` | 12×41, 22×30, 37×15, 31×12, 9×8 |
| `BusinessProcesses/Задание/Forms/ДействиеВыполнить` | 12×38, 22×31, 37×15, 31×8, 9×7, 2×1 |
| `Documents/Лот/Forms/ВыигранныеЛоты` | 12×25, 22×18, 31×10, 37×6, 5×6, 55×2 |

A field is wrapper `37`, a button `31`, a group or command bar `22`, a
decoration or tooltip `12`, a table `55`, an attribute `9`.

## What the compiler writes

`format_form_layout_new_label_field_item` and its siblings emit wrapper `48`
records of about ten members:

```
{48,{<id>,<uuid>},0,0,0,1,"<name>",1,0,{<title>}<events>}
```

Wrapper `48` does not occur in any of the four stored bodies above. The
platform's own field record carries the full property tail -- the same tail the
export reader walks by reverse offset -- and the compiler's does not write it.

So the compiler is a *creator*: it builds the smallest record that names the
item, and leaves the platform to fill the rest on first save. That is a
different goal from the export's, which reproduces the stored bytes exactly.

## What this means for the remaining work

Closing the census families inside the current creator widens what can be
loaded; it does not move the output toward the bytes the platform stores. A
load that round-trips -- export a configuration, load it into an empty
database, export again and get the same tree -- needs a body *writer* for the
8.3.27 layout: wrapper 37/31/22/12/55 records with their full tails, in the
platform's own order, which is the inverse of the reader the export side
finished.

The one measurement that says this is tractable rather than open-ended: the
extended tooltip, which blocks more forms than anything else (10 174 of 13 044
in ERP УХ), is stored as a *single* constant shape. All 85 tooltip records
across two unrelated forms normalise to

```
{12,{<id>,<owner uuid>},0,0,0,0,"<name>",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},
 {7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},
 {3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},
 0,0,1,0,0,1,0,3,3,0,0}
```

parameterised only by the item id, the owner uuid and the name. Most families
will be like this: a fixed template plus the few slots the XML declares, each
needing its own measurement against stored bodies.

## Status

The export side is complete on both corpora. The load side is a body writer
that has not been built, and the census in
`form-body-blockers-20260921.md` is its work list.
