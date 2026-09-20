# Tasks: export the active dynamic generation

- [ ] 1. Read the storage table's `DynamicallyUpdated` record once per run and resolve the active generation, treating its absence as "none".
- [ ] 2. Build every row query on a storage-table expression that publishes `<base>_dynupdate_<active>` as `<base>`, hides the plain row it overrides, and drops every other generation's alias.
- [ ] 3. Apply the same substitution to the file-name inventory, including the `versions` record.
- [ ] 4. Re-point `is_superseded_generation_owner` at "not the active generation" rather than "carries the infix".
- [ ] 5. Unit-test the substitution against an alias set with one active and one superseded generation.
- [ ] 6. Re-run the BSP parity on the database holding the active generation and record the evidence.
- [ ] 7. Re-run the ERP УХ parity, which has two superseded generations and no active one, and confirm no change.
