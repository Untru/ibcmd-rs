# Tasks: export the active dynamic generation

- [x] 1. Read the storage table's `DynamicallyUpdated` record once per run and resolve the active generation, treating its absence as "none".
- [x] 2. Build every row query on a storage-table expression that publishes `<base>_dynupdate_<active>` as `<base>`, hides the plain row it overrides, and drops every other generation's alias.
- [x] 3. Apply the same substitution to the file-name inventory, including the `versions` record.
- [x] 4. Re-point `is_superseded_generation_owner` at "not the active generation" rather than "carries the infix".
- [x] 5. Unit-test the substitution against an alias set with one active and one superseded generation.
- [x] 6. Re-run the BSP parity on the database holding the active generation and record the evidence.
- [x] 7. Re-run the ERP УХ parity, which has two superseded generations and no active one, and confirm no change.

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
