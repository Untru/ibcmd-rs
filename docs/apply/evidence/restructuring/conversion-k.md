# Case k: data conversion by the native apply (measured)

Table `_Reference2598` (`Catalog.КлючевыеОперации`, 716 rows), columns added in case h; five rows were
seeded with SQL before the apply (`_Fld11045 = 0x01`, `_Fld11048 = 123456`, `_Fld11051 = N'abcdefghij'`).
Tree edit (`edit_cases_k.py`), staged with our `--base-free` import, applied by native
`config apply --force --dynamic=disable` (exit 0, 175 s on a busy machine; the only platform message about
the object: "Объект изменен: Справочник.КлючевыеОперации", no warning about data loss).

| attribute | change | SQL type before -> after | value before -> after |
|---|---|---|---|
| `ДемоСтрокаНеогр` | String unlimited -> String(5) | `nvarchar(max)` -> `nvarchar(5)` | `abcdefghij` -> `abcde` (truncated) |
| `ДемоЧисло` | Number(10,0) -> Number(5,0) | `numeric(10,0)` -> `numeric(5,0)` | `123456` -> `99999` (saturated, no error) |
| `ДемоБулево` | Boolean -> String(10) | `binary(1)` -> `nvarchar(10)` | `0x01` -> `Да`, `0x00` -> `Нет` |

How the data moved: **not** by `INSERT ... SELECT` with SQL casts, as for a new column. The platform created
`_Reference2598NG` (six indexes, then dropped as usual), read the old table in pages
(`SELECT TOP 100 T1._IDRRef, ..., T1._Fld11045, ... FROM dbo._Reference2598 T1 ... ORDER BY ...`), converted
the values in the 1C engine and wrote them with `insert bulk dbo._Reference2598NG(...)` (batches of 100 rows).
The conversion therefore follows the platform's rules per pair of types (truncation, saturation, language
dependent text of a boolean), which an own implementation has to reproduce or refuse.

The same run shows the other table shape: a flat, non-separated catalog has an inline
`_IDRRef binary(16) not null primary key` in the `create table`, and its indexes have no separator columns.
