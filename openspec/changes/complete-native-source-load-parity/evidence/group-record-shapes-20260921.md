# Every group kind is one structure with scalar slots, 21.09.2026

The body-layout note said the writer has to produce the records the platform
stores. This one says how much structure those records actually have, measured
over the whole ERP УХ form corpus rather than a handful of forms.

Every `Form.xml` of the tree names a form body row; all 12 515 of them were
read out of `ibcmd_rs_uha_8327_parity2_20260920` with
`mssql-dump-config --inflate --file-name-list`, giving 25 030 inflated parts.
`F:\ibcmd\lab\tools\census-group-payloads.py` then took every `{22,…}` group
record, split its top-level members, and counted the distinct shapes of the
payload tuple that follows the common frame, with strings and uuids masked.

## The common frame

```
{22,{<id>,<item namespace>},0,0,0,<kind>,"<name>",<title>,<tooltip title>,
 0,1,0,<?>,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,
 <kind payload>,
 <child count>,(<child group uuid>,<child record>)…,
 1,0,1,<extended tooltip>,0,3,3,0}
```

`<kind>` 8 is a context menu and 9 an auto command bar -- both already written.
The rest are the group kinds:

| kind | records | distinct payloads | payload structure |
|---|---|---|---|
| 5 | 68 836 | 1 223 | `{29,…}` -- 28 members |
| 6 | 21 795 | 184 | `{2,{0},2,0}` -- 4 members |
| 4 | 12 988 | 236 | `{18,…}` -- 20 members |
| 1 | 10 989 | 49 | `{7,…}` -- 9 members |
| 2 | 7 177 | 49 | `{2,…}` -- 12 members |
| 7 | 5 474 | **1** | the scalar `0` |
| 3 | 5 317 | 13 | `{4,…}` -- 6 members |
| 0 | 3 481 | 192 | `{1,…}` -- 3 members |

## What the counts mean

Each kind has exactly **one** payload structure. The hundreds of "distinct
shapes" are combinations of a handful of scalar slots inside that one
structure: kind 5's 1 223 shapes are the `{29,…}` tuple with different values
in about six of its twenty-eight members, and kind 3's thirteen are
`{4,<a>,{0,1,0},2,0,<b>}` with two.

That is the difference between an open problem and a mechanical one. The
writer does not have to discover a record per form; it has to fill one
structure per kind, and the slots it must fill are the same slots the export
reader already names by reverse offset -- `FormUsualGroupSchema`,
`FormPagesSchema` and their siblings read them today.

## The shape of the remaining work

For each item kind: take the reader's schema, invert it into a writer, and
prove the writer against the stored records of that kind in this dump. The
five records already written -- the tooltip in both its shapes, the field
context menu, the `{37,…}` field, the `{31,…}` standard-command button and the
empty auto command bar -- were proved exactly that way.

The dump this note is built on is the evidence base for the rest:
`F:\ibcmd\lab\form_bodies\Config_inflated`, 25 030 files, every form body of
ERP УХ 3.3.3.3.
