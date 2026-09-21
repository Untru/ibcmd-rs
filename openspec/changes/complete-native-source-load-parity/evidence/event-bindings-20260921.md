# Event bindings, measured over ERP УХ

Date: 2026-09-21. Corpus: the native 8.3.27.2214 export of ERP УХ,
`E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native`,
against the inflated form bodies of the same database in
`F:\ibcmd\lab\form_bodies\Config_inflated` (12 515 rows, one part each).

Tool: `F:\ibcmd\lab\tools\map-events.py`. Report:
`F:\ibcmd\lab\events-map-full.txt`.

## What was measured

Every `<Events>` the source names, wherever it names it: on the form itself, on
an item, on an extended tooltip. The tool finds every events-shaped tuple in the
body, decides which item record owns it by containment, rebuilds it from the XML
alone and compares it to the stored bytes.

```
forms 12507 | xml events 101946 | claimed by a tuple 100958 (99.03%)
            | tuples 63161 | exact 63161 (100.00%)
```

**63 161 of 63 161 tuples rebuild byte for byte.** The 988 events no tuple
claimed are an artefact of the tool, not of the shape: it pairs an event to a
tuple by the handler's text, and a form that calls the same procedure from two
different events leaves the assignment ambiguous. Nothing about those events is
unknown; they are simply not counted twice.

## The shape

```text
{ N, (<uuid>,"<handler>") x N, 1, 0, (<uuid>,0,<M>,("<handler>",1) x (M-1)) x N }
```

* `N` counts the **distinct events**, in the order the source first names each.
* Each event heads a pair with its uuid and its first handler.
* `1,0` separates the heads from the tails.
* Each tail repeats the uuid, then `0`, then `M`, the number of handlers that
  event carries.
* An owner with no events writes `{0,1,0}`.

`M` is above 1 when a configuration extension adds its handler beside the base
one. The two are written as two `<Event>` elements of the **same name**, so the
group -- not the element -- is the unit. One example, whole:

```xml
<Events>
  <Event name="OnChange">НаборДанныхБазыРаспределенияПриИзменении</Event>
  <Event name="OnChange">Расш1_НаборДанныхБазыРаспределенияПриИзмененииПосле</Event>
</Events>
```

```text
{1,fe115cc8-9e33-4684-a166-bd5136fe7a9f,"НаборДанныхБазыРаспределенияПриИзменении",1,0,
 fe115cc8-9e33-4684-a166-bd5136fe7a9f,0,2,"Расш1_НаборДанныхБазыРаспределенияПриИзмененииПосле",1}
```

Reading the two as two events gives `N=2` and does not rebuild; reading them as
one event with two handlers gives the stored bytes. That is what closed the last
17 records and took the measurement from 99.97% to 100.00%.

## An event's uuid is not its name

The uuid belongs to the **kind that declares the event**: `OnChange` of an input
field (`fe115cc8-…`) is not `OnChange` of a table. Keyed by `(tag, name)` the
corpus gives 127 pairs, of which 125 are unambiguous.

The two that are not are `BeforeWrite` and `BeforeWriteAtServer` of a form, and
what decides them is the class of the form's **main attribute**, because that is
what picks the form extension declaring the event:

| main attribute class | `BeforeWrite` | `BeforeWriteAtServer` |
|---|---|---|
| `DocumentObject` | `8a5894c9-…` | `8f42e083-…` |
| `CatalogObject`, `ChartOfAccountsObject`, `ChartOfCalculationTypesObject`, `ChartOfCharacteristicTypesObject`, `ExchangePlanObject`, `BusinessProcessObject`, `TaskObject`, `ConstantsSet`, `AccountingRegisterRecordSet`, `InformationRegisterRecordSet`, `InformationRegisterRecordManager` | `9cc34712-…` | `bf0ac0e1-…` |

Keyed by `(main attribute class, tag, name)` the corpus has **zero** names that
two uuids share.

A name the platform itself could not spell is written as the event's uuid in the
`name` attribute; it stands for itself. Ten such names appear.

## Where the events sit

An item does not keep all of its events in one place. An input field writes
`OnChange` in its own record and `AutoComplete`, `StartChoice`,
`ChoiceProcessing`, `Clearing` and the rest inside its editor payload. That is
routing, and it belongs to each item's writer; the shape above is the same in
both places.

## What the writer does

`format_native_events` in `src/compiler/bodies/form_native.rs` builds the tuple
from `(owner tag, main attribute class, events)`, with `FORM_EVENT_UUIDS`
holding the 145 measured entries. A name the table does not hold, and a main
attribute class the corpus never showed declaring a split event, are **refused**
rather than defaulted: writing the wrong event uuid would bind a handler to the
wrong event, which no later check would catch.
