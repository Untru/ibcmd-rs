# Tasks: file infobase backend

- [x] 1. Measure the 8.3.8 layout on native files: header, objects, blobs, table descriptions, records, free chains, B-tree pages and keys; record the evidence (`evidence/1cd-format.md`, #372).
- [x] 2. Add `ibcmd-filedb`: a fail-closed reader of the configuration tables and an index verifier; check every native index key for key (#372, #373).
- [x] 3. Add the writer: rebuild the record, blob and index objects of one table in a locked copy, read it back, keep a backup, rename; check `config apply` and `config export` of the platform on files it wrote (#374).
- [x] 4. Export through the offline-rows seam with the SQL rules for a completed stage; `mssql-dump-config --file-infobase` (#373).
- [x] 5. Import: base-free or `cf load` onto the target's rows, a `deleted` row, the stage guard, parts of 10 000 000 bytes (#374).
- [x] 6. Drop-in `--db-path`/`--database-path` for export and import; conflicts with the DBMS options; refusal of apply, save and import files; the platform from the settings by directory (#375).
- [x] 7. Refuse an open infobase before any work; check against a running standalone server both ways.
- [x] 8. Keep an unused data parameter that is a user setting in form list settings (БСП on 8.5.1.1150).
- [x] 9. Fixtures written by 8.5.1.1150 and offline tests: the export against the native export, imports read back through the export, broken files.
- [x] 10. Measure the БСП round trips (`evidence/bsp-8-5-roundtrip-20261021.md`).
- [x] 11. Measure the `ru_RU` collation and write Russian infobases; refuse other locales and indexes that do not match the formula (review).
- [x] 12. Hold the write lock from the read of `CONFIG` to the write; keep the data file's mode and owner; check page isolation; name work and backup files after the data file; refuse corrupted lengths before allocating (review).
- [ ] 13. Measure ERP УХ and 8.3.27 file infobases (not available on the measuring host).
