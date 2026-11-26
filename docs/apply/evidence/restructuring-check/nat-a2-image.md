# The whole native image of the ddl track's case a2 (#404)

The ConfigSave that the platform 8.3.27.2214 own `config import` staged for the ddl track's case a2 (an attribute
added to `Catalog._ДемоПартнеры`) into a restore of the corpus `bsp_native_20260923.bak`: 9 842 rows (9 839 names).
The rows are in the lab, `F:\ibcmd\lab\04\restructure-check\fixtures\nat_a2\ConfigSave` (saved from
`ibcmd_rs_04_rcheck_nat_a2` before it was dropped), and the active Config it is compared with in
`fixtures\nat_a2_base\Config` (all 9 847 rows of the corpus restore). Both are read by
`cargo test -p ibcmd-rs --lib --no-default-features apply_check::s1h -- --ignored --nocapture`
(`s1h_corpus_tests.rs`, 90-250 s in the debug profile).

## Before (checkpoint 1 of #338, docs section 8, finding 9)

400 reasons: 1 real, 396 `unknown` "the row differs (a -> b bytes) but both sides decode to the same XML", 2 for the
Configuration `{68}` row ("fields 26 and 43 hold 80324 and 80327"), 1 for the `deleted` row.

## After

```
Реструктуризация нужна: применять штатным config apply.
Файлов в конфигурации: 9838 -> 9835; строк в ConfigSave: 9839; добавлено 0, удалено 3; описаний сравнено 517, тел с данными сравнено 28.
Объектов с отличиями: 1 (из них с причинами: 1).
Причины (1):
  [structure] Catalog._ДемоПартнеры: ChildObjects/Attribute[ДемоНовыйРеквизит]: added (a column is added or dropped)  (5eab8a1b-070f-4dcf-bdcc-a259c62c3693)
Безопасные изменения (6):
  ExchangePlan.<six plans>: Content: the same items in another order
S1 принимает всё: операций 1.
  add-attribute: Catalog._ДемоПартнеры
```

`stats.format_upgrades` is 516: the 396 rows that decode to the same XML in the same format, the 119 that decode
only in the newer format (they were notes before), and the Configuration row of the `{68}` shape. The reason, as data:
`rule: column-added-or-dropped`, `kind: Catalog`, `path: ChildObjects / Attribute[ДемоНовыйРеквизит]`, `op: added`.
The row `deleted` is `0` (BOM and one digit) and is accepted.

## Through the command, on the database

The same ConfigSave restored from the ddl track's `ibcmd_rs_04_ddl_bsp8327_a_a2_staged.bak` as
`ibcmd_rs_04_rcheck_nat_a2` (native lock not needed: the command only reads), debug build:

```
ibcmd-rs mssql-apply-check --database ibcmd_rs_04_rcheck_nat_a2 --s1 --fail-on-restructuring
```

exit 10 (a restructuring is needed: the attribute), 8.6 s, the report above. With `--json --s1` the output is
`{ "verdict": {...}, "s1": {...} }`; the reason is `rule: "column-added-or-dropped"`, `kind: "Catalog"`,
`path: [ChildObjects, Attribute[ДемоНовыйРеквизит]]`, `op: "added"`, and
`s1.operations: [{ "operation": "add-attribute", "object": { "kind": "Catalog", "name": "_ДемоПартнеры",
"row": "5eab8a1b-070f-4dcf-bdcc-a259c62c3693" }, "attribute": "ДемоНовыйРеквизит" }]`, `s1.refusals: []`,
`stats.format_upgrades: 516`.

## What the proof does not let through (same run, one thing changed)

| change to the image | result |
|---|---|
| the class id `3b10624f-1e3d-495d-8093-25225efc5313` that the importer adds to the property lists changed by one digit, in one of the 516 rows | 1 reason `row-format-upgrade-unproven` naming that row and the value; `format_upgrades` 515; S1: `row-format-unproven` |
| `deleted` holds `{1,"some.file"}` | 1 reason `deleted-row-not-empty`; S1 refuses |

## The differences that were proven

Item by item alignment of the stored and the staged brace trees (`apply_check::upgrade`, docs section 3.6): the head
of a list grows by one (record version), a counter grows by one while an entry is inserted, the entries inserted are
the platform's defaults (`0`, `1`, `5`, `3b10624f-...`, `{1,<nil uuid>}`, `{"#",502b7765-...,{502b7765-...,0}}` and
`...,2}`, and for the Configuration row `{41,0}`, `{a7641777-...,0}`, `{-1036481104}`), an empty string becomes `"5"`,
and in the Configuration row an integer in `80300..80399` grew (the extension compatibility mode). 516 of the 517
descriptors that differ in bytes are exactly that; the 517th is the attribute.
