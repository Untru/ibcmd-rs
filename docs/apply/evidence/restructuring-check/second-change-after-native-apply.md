# A second change to a catalog after a native apply (rcheck-6, task 1)

Finding of the ddl track (S1-B/C twins): after the native apply restructured a catalog, the next restructuring change
to the same catalog was `unknown` for `apply_check::s1::classify`. Reproduced on a БСП 8.3.27 clone
(`ibcmd_rs_04_rcheck_twin1`, restored from `bsp_native_20260923.bak`, dropped afterwards), platform 8.3.27.2214,
`--user=Администратор`, the lab's `native` lock held for each command. The reference tree is only read; the staged
files are the reference `Catalogs/_ДемоПартнеры.xml` with one edit each.

| step | command | result |
|---|---|---|
| 1 | `infobase config import files --base-dir=<stage1> --partial Catalogs/_ДемоПартнеры.xml` (stage1: an attribute `Rcheck1`, String(30), added) | exit 0, 7.6 s, ConfigSave 4 rows (the descriptor, `root`, `version`, `versions`) |
| 2 | `infobase config apply --force --dynamic=disable --session-terminate=disable` | exit 0, 61 s, `Реструктуризация Справочник._ДемоПартнеры`; ConfigSave empty, `Config` 9 841 rows |
| 3 | the row `5eab8a1b-070f-4dcf-bdcc-a259c62c3693` of `Config` | 36 275 bytes inflated (`{56,...}`) before, 38 328 after: **byte-equal to the staged row**, `{57,...}` |
| 4 | `import files --partial` of stage2a (`Rcheck2`, String(20), added on top of stage1) | exit 0, 11.7 s |
| 5 | `mssql-apply-check --database ... --s1` before the fix | `[unknown] Catalog._ДемоПартнеры: the Catalog row changed and cannot be decoded: slot 1 (Tag(56, 57, 57)): record version 57, the compatibility mode stores 56`; `S1 отказывает ... unknown-step [row-undecodable]` |
| 6 | the same after the fix | `[structure] ChildObjects/Attribute[Rcheck2]: added`, `S1 принимает всё: операций 1. add-attribute: Catalog._ДемоПартнеры` |
| 7 | stage2b (`Rcheck1` `Length` 30 -> 60), staged the same way | `ChildObjects/Attribute[Rcheck1]/Properties/Type/StringQualifiers/Length: 30 -> 60`, S1 `widen-string` |
| 8 | stage2c (`CodeLength` 9 -> 12) | `Properties/CodeLength: 9 -> 12`, S1 `отказ property-outside-s1` |
| 9 | `--tree stage1 --partial` against the database (the tree that is the active catalog) | `Реструктуризация не нужна`, 0 differing descriptors (the export of the `{57}` row equals the file) |
| 10 | `--tree stage2a --partial` | `Attribute[Rcheck2]: added`, S1 `add-attribute` |

Cause: `Decoder::decode` reads a row in the compatibility mode of the configuration (8.3.24 stores catalog tag 56);
the staged side had a fallback to the newest mode, the stored side had none, and a promoted row is stored in the
format of the platform that staged it. Fix in `apply_check::plan` (`Decoder::decode_stored`, `export`) and
`check.rs` (`Describe::describe_stored`); the unit tests compile the rows of both formats with the model's writer from a
synthetic catalog (`record_format_tests.rs`), no corpus bytes.
