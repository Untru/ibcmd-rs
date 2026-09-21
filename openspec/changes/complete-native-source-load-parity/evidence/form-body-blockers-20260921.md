# What stands between a source tree and a load, 21.09.2026

`audit-form-body-blockers <root>` walks every `Form.xml` of a source tree,
runs it through the base-free form body model, and reports the reasons it
refuses, collapsed to their shape (the object names a message quotes are
evidence, not a category). Two phases:

- `parse` -- the Form XML reader itself refused the document;
- `model` -- it read the document and the base-free model refused the shape.

A form is compilable only when it raises no reason at all, so closing one
reason usually does not make a form compile; it uncovers the next.

## Starting point, both corpora

| | ERP УХ | BSP |
|---|---|---|
| `Form.xml` files | 13 044 | 1 108 |
| compilable today | 86 | 28 |
| blocked | 12 958 | 1 080 |

## After the first fix: the excluded-command name table

The reader knew four `<ExcludedCommand>` names. The export's own uuid→name
table names 72, of which 63 map back to exactly one uuid; teaching the reader
those 63 removed every `parse` refusal for `Abort` (2 689 ERP УХ forms),
`Help`, `Write`, `WriteAndClose`, `Activate`, `CreateFolder`, `CancelSearch`
and the rest. Nine names -- `ChangeHistory`, `Copy`, `CreateInitialImage`,
`Delete`, `Post`, `ReadChanges`, `SetDeletionMark`, `UndoPosting`,
`WriteChanges` -- are spelled for two or three uuids and stay refused, because
which uuid a form stores depends on its family:
`BusinessProcesses/Задание/Forms/ДействиеВыполнить` stores `68baa1bc-…` for
`Copy` while an ordinary list form stores `342c531d-…`.

Compilable forms moved 86 → 87 (ERP УХ) and stayed at 28 (BSP), which is the
expected shape of the work.

## The remaining reasons, ERP УХ (forms raising each)

| forms | phase | reason |
|---|---|---|
| 10 174 | model | Form item uses an unsupported decoration/font/border creation facet |
| 4 600 | model | Form attribute requires numeric id, name and a typed pattern |
| 4 463 | model | unsupported base-free element `LabelDecoration` |
| 3 837 | model | item uses strict-schema properties without an evidenced creation layout |
| 2 075 | model | unsupported base-free element `SpreadSheetDocumentField` |
| 1 560 | model | root property-bag/report facets require a separately evidenced template |
| 973 | parse | `<ExcludedCommand>Copy` (family-ambiguous uuid) |
| 946 | model | Form root event has no unambiguous platform event UUID |
| 862 | model | unsupported base-free element `RadioButtonField` |
| 724 | model | unsupported base-free element `PictureField` |
| 502 | parse | conditional-appearance nested filter outside the one-command cohort |
| 348 | parse | `<ExcludedCommand>Post` (family-ambiguous uuid) |
| 144 | parse | DCS settings container with more than one direct element child |
| 126 | model | unsupported base-free element `HTMLDocumentField` |

BSP raises the same families in the same order at a tenth of the scale.

Full reports: `bsp-form-blockers-20260921.json`,
`uh-form-blockers-20260921.json`.

## What this says about the work

Load parity is the inverse of the reader the export side finished: each
remaining family needs its own platform evidence -- which slot a facet
occupies, in what order, and which uuid a name resolves to for which owner
family -- measured against stored bodies rather than inferred. The census is
the map; it is not the work.

Two of the families above are *ambiguity*, not absence: a name the export can
read is not always a name the compiler can write, because several names are
spelled for more than one uuid. Those need a family-resolved table built from
stored root command sets, not a wider name list.
