# Tasks: export the active dynamic generation

- [x] 1. Read the storage table's `DynamicallyUpdated` record once per run and resolve the active generation, treating its absence as "none".
- [x] 2. Build every row query on a storage-table expression that publishes `<base>_dynupdate_<active>` as `<base>`, hides the plain row it overrides, and drops every other generation's alias.
- [x] 3. Apply the same substitution to the file-name inventory, including the `versions` record.
- [x] 4. Re-point `is_superseded_generation_owner` at "not the active generation" rather than "carries the infix".
- [x] 5. Unit-test the substitution against an alias set with one active and one superseded generation.
- [x] 6. Re-run the BSP parity on the database holding the active generation and record the evidence.
- [x] 7. Re-run the ERP УХ parity, which has two superseded generations and no active one, and confirm no change.
- [x] 8. Build the storage-table expression from the generation history alone, so that its text does not grow with the number of aliases (SQL error 8621 on a database updated online many times; #390).
- [x] 9. Leave out the published names the active `versions` record does not list (rows an online update that removed an object or a module leaves behind).
- [x] 10. Publish the main configuration in `infobase config export`: the rows a completed import staged in `ConfigSave` in place of the `Config` rows of the same names, and the staged `versions`.
- [x] 11. Read a `versions` row of a `Config` or `ConfigSave` table by its pairs, and keep the platform's sentinel for the command of a constant that uses no standard commands.
- [x] 12. Re-run the database with 17 generations and 127 885 aliases against the native export, the БСП database with an active generation and the corpora, and record the evidence (`evidence/many-generations-and-main-configuration-20260929.md`).
- [x] 13. List in `ConfigDumpInfo.xml` the first `count` pairs of a `versions` row, as the platform does after a native stage, and read the extension compatibility of a staged `{68}` Configuration row from field 43 (#411).

## Notes

Task 4 needed no code change. The substitution happens in the storage-table
expression every query reads from, so no alias of the active generation ever
reaches a row set under its alias name; what still carries the infix there is a
superseded generation, which is exactly what `is_superseded_generation_owner`
already decides. A database with superseded generations and no active one --
ERP УХ -- keeps that path byte for byte.

The history is applied in order rather than as a single generation: a
transition writes only the rows it changes, so a published name several
generations carry is read from the newest one that carries it.

Tasks 8 to 12 (#390). A database updated online many times holds a hundred
thousand aliases; the first expression listed every one of them and the
optimizer ran out of stack. The published configuration is what the active
`versions` record lists, and it is not always the one `Config` holds: the
platform's export writes the *main* configuration, so the rows a completed
import staged in `ConfigSave` win over the `Config` rows of the same names.
