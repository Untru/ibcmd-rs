# The body frame and its sections, measured over ERP УХ

Date: 2026-09-21. Corpus: the inflated form bodies of ERP УХ,
`F:\ibcmd\lab\form_bodies\Config_inflated`.

Tools: `F:\ibcmd\lab\tools\census-frame.py`, `census-attributes.py`. Reports:
`frame-census.txt`, `attributes-census.txt`.

## Ten members, in every body

```
frame widths: [(10, 12507)]
```

```text
{4,
 <root {50,…} record>,
 <module text>,
 <attributes section>,
 <parameters section>,
 <commands section>,
 <conditional appearance>,
 <one more appearance section>,
 0,
 0}
```

Member 0 is always `4`; member 1 is always the root record; members 8 and 9 are
always `0`. The existing base-free writer emits **seven** members
(`{4,<layout>,"",{0},{0,0},{0,0},{0}}`), which is the structural reason its
output is not what the platform stores.

Members 4 and 5 are recognisable by what they carry: the parameters section
holds `{0,"<name>",…}` entries, the commands section holds `{9,{<id>,`
`409b9a53-7f7e-4178-86c1-33176c7c7a7a},…}` records -- the command namespace.

## The attributes section carries the settings composer

```text
{4,<count>,<attribute record> x count,
   <appearance count>,<appearance entry> x m,
   0,
   <settings blob>}
```

The marker is `4` in all 12 507 bodies. After the attribute records come the
form's conditional appearance entries, a `0`, and a base64 blob. In 9 612 bodies
there is no appearance at all, so the section ends `…,0,0,<blob>`.

Only **76 forms have no attributes**, so a writer for this section is a writer
for the `{9,…}` attribute record.

## The settings blob is not in the source

The blob is a UTF-8 `<Settings>` document with a BOM and CRLF line endings, and
two spellings account for 11 842 of the 12 507 bodies:

| bytes | shape | forms |
|---|---|---|
| 766 | `<Settings …/>`, self-closed | 6 782 |
| 810 | `<Settings …>` with `<outputParameters/>` | 5 060 |

Which of the two a form gets **cannot be read from its source**. Correlating
against the class of the main attribute separates nothing -- both spellings
appear for forms with no main attribute and for dynamic-list forms alike -- and
comparing the element vocabulary of the two cohorts finds *no* element that is
even half again as common in one as in the other.

That is a fact about the corpus, not a gap in the reading: the platform wrote
whichever spelling the form carried when it was last saved, and the export does
not put it in the XML.

**What it means for the round trip.** The load side cannot reproduce the
original blob, and it does not have to. The closing criterion is
export → load → export byte-identical *to the second export*, so the writer
picks one canonical spelling -- the self-closed `<Settings/>`, which is what a
newly created form carries -- and the round trip closes on itself. Any property
that the export does not put in the source is in the same position, and this is
the first one measured.
