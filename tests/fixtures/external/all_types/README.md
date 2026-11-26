# `Обработка все типы` — one attribute per platform type

Two builds of the same external data processor, whose attribute list has one
attribute per platform type (Chart, GanttChart, Planner, GeographicalSchema,
FlowchartContextType, TextDocument, PDFDocument, the data-analysis enums, ...),
each named after the type it holds. This is the corpus that pins the
`d5p1:`/`d7p1:` type spellings and the Russian type names quoted in the comments
of `DATA_PROCESSOR_TYPES` (`crates/ibcmd-schema/src/lib.rs`).

- `inputDesigner.epf` — saved by the Designer, 19 182 bytes, SHA-256
  `004D924DEAA5510115CABF36AB76BF993004BA96CC6B244D3A293E152480553D`. Its
  references are stored as type ids.
- `inputIBCMD.epf` — the same object rebuilt from that XML by the platform,
  18 063 bytes, SHA-256
  `0B656183108D14BCB2034ABAA770987260523846C56589DF5289E5DAEC1F2CE6`. Its
  references come back as names.

Both are checked in so a reviewer can run every step below without the stand
corpus.

## Acceptance scenario

The scenario is the round trip both tools must agree on, in both directions.
Only the **exported XML trees** are compared, never the container bytes: a `.epf`
carries timestamps and ids the tools are free to differ in, and the contract is
about content.

```powershell
$ib   = "C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe"
$rs   = ".\target\debug\ibcmd-rs.exe"
$fx   = "tests\fixtures\external\all_types"
$work = "$env:TEMP\all-types"; New-Item -ItemType Directory -Force $work | Out-Null
```

### 1. `inputDesigner.epf` → XML, both tools

```powershell
& $ib config export "--file=$fx\inputDesigner.epf" "$work\plat\ВнешняяОбработка1"
& $rs cf export --overwrite --platform 8.3.27.2214 "$fx\inputDesigner.epf" "$work\rs"
```

Both dumps have the same layout relative to their roots (`$work\plat` holds
`ВнешняяОбработка1.xml` and `ВнешняяОбработка1\`; `$work\rs` the same), so the
two trees are compared file by file. They agree except for the two known
relative Planner-period dates in the Designer form (see below).

### 2. XML → `.epf`, both tools

```powershell
& $ib config import "--out=$work\plat.epf" "$work\plat"
& $rs cf bootstrap --base-free --platform 8.3.27 "$work\plat" "$work\rs.epf"
```

The two `.epf` files are compared by content, not by container bytes: export
each with the same tool and diff the trees.

```powershell
& $ib config export "--file=$work\plat.epf" "$work\plat2\ВнешняяОбработка1"
& $ib config export "--file=$work\rs.epf"   "$work\rs2\ВнешняяОбработка1"
```

`$work\plat2` and `$work\rs2` **identical.**

### 3. That `.epf` → XML again, both tools

```powershell
& $ib config export "--file=$work\rs.epf" "$work\plat3\ВнешняяОбработка1"
& $rs cf export --overwrite --platform 8.3.27.2214 "$work\rs.epf" "$work\rs3"
```

`$work\plat3` and `$work\rs3` **identical**. Comparison with the Designer input from step 1 still allows
only the two documented relative Planner-period dates; step 1 is not a
byte-identical platform comparison.

### Current status

| step | measured |
|---|---|
| 1 | equal except the two planner-period dates below: `ВнешняяОбработка1.xml` and `Forms/Форма.xml` 0 differing lines, `Forms/Форма/Ext/Form.xml` 2 |
| 2 | equal on content: the platform writes `inputIBCMD.epf` (18 063 bytes) and `cf bootstrap --base-free` writes 17 623; exported by the same tool, both trees are identical |
| 3 | equal: 0 differing lines on all three files |

`inputIBCMD.epf` as an input is 0/0/0 too.

## Rebuilding the XML with the platform

The reference dump must come from a fresh platform run of the same build
(`ibcmd.exe` 8.3.27.2214); older XML exports of this object predate the
type-spelling rules and disagree for that reason alone.

`config export` names the root file after the **directory** it is given
(`<work>\ВнешняяОбработка1.xml`) and puts the object's files beside it, so
`config import` is pointed at `<work>`, not at `<work>\ВнешняяОбработка1`.
Exporting `inputIBCMD.epf` again reproduces all three files of the Designer dump
byte for byte, which is what makes it a usable second input.

## Known differences from the platform dump

Designer build only: two planner-period lines in `Forms/Форма/Ext/Form.xml` —
the platform writes today's date into the period where the object stores
2026-10-14. The period is a relative one, so the platform resolves it at export
time and its own dump changes daily; these two values are compared as the one
known difference rather than byte for byte.

The configuration references an external object carries (`cfg:CatalogRef.…` and
the like) are written as `<v8:TypeId>` for exactly the same reason the platform
writes them that way: the name exists only in the object's own `copyinfo`, and
a name it cannot resolve is not a name. Platform types the built-in table knows
(`ent:AccountingRecordType`, `d5p1:Chart`, …) stay names in both dumps.

The Rust CLI currently returns exit 2 for the initial `inputDesigner.epf`
export: its `versions` record lists UUID
`6c422377-2355-44c5-ad7a-f49f72d7b30d`, which is absent from the container.
The three source XML files are emitted, but `ConfigDumpInfo.xml` is refused.
A local offline diagnostic on 2026-10-23 confirmed that bootstrapping those
three files and exporting the result returns exit 0 and preserves all three
files byte for byte. `inputIBCMD.epf` completes the entire Rust round trip
with exit 0. XML content equality therefore does not imply that the initial
Designer-container export command succeeded; the inventory defect remains
open and the fail-closed validation is retained.
