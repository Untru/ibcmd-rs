# File infobases: export and import with `--db-path`

## Why

`ibcmd infobase config export|import --db-path=<dir>` works on the file
infobases most developers and test stands use, and ibcmd-rs refused every
such command line ("файловые информационные базы не поддерживаются"). The
milestone 0.10 — #372 (the format), #373 (export), #374 (import), #375 (the
drop-in syntax) — asks for both operations without the platform.

## What changes

- A new crate `ibcmd-filedb` reads the 8.3.8 format of `1Cv8.1CD` (pages,
  objects, blobs, table descriptions, records, B-tree indexes) and replaces
  every row of one configuration table. The format and every rule the writer
  keeps are evidenced in `evidence/1cd-format.md`.
- The export reads `CONFIG`, folds in a completed import stage of
  `CONFIGSAVE` as it does for SQL Server, and runs the existing export on
  those rows (`mssql_dump::file_infobase`, through the offline-rows seam
  `--rows-dir` already uses). `mssql-dump-config --file-infobase` is the
  research form.
- The import stages the whole configuration in `CONFIGSAVE`, as the
  platform's own import into a file infobase does: compiled from the tree
  when the target holds no row of the tree's configuration
  (`mssql::base_free_cf`), else loaded onto the target's rows the way
  `cf load` loads a tree onto a `.cf` (`crate::load`); plus a `deleted` row
  naming the rows the configuration lost. The stage guard checks the state
  against the tree before anything is written.
- The write locks the data file for the whole import, works in a copy with
  the original's mode and owner, checks that no page outside the table
  changed and that the table and its index read back, keeps the original as
  `<data file>.ibcmd-rs.bak` and renames the copy into place. An infobase the
  platform has open is refused before any work; the platform cannot open one
  ibcmd-rs is writing.
- Index keys follow the infobase's locale: `en_US_POSIX` and `ru_RU` are
  measured; an infobase in another locale is read but not written, and a
  write first checks the infobase's own indexes against the formula.
- The drop-in grammar takes `--db-path`/`--database-path` for `config
  export` and `config import`; it contradicts `--dbms`, `--db-server` and
  `--db-name`. `config apply`, `config save` and `config import files` still
  refuse a file infobase, with a message that says what does work. The
  settings can name the platform of a file infobase by its directory (a
  database entry without a server).
- `ibcmd-xml`: an unused, nil data parameter of a dynamic list is left out
  of the form's list settings only when it is not a user setting (has no
  `userSettingID`). The platform 8.5.1.1150 keeps such an item (БСП
  `Catalogs/СценарииОбменовДанными/Forms/НастройкаРасписанияОбменовДанными`);
  the rule that drops it came from an item without one (ЛИМС КОРП, 8.3.27).

## Not in this change

- `config apply` of a file infobase (the platform's apply runs on the
  staged result), extensions (`CONFIGCAS`), `import files`, `config save`.
- Writing a name outside the evidenced collation range: such a row is
  refused, not guessed.
- 8.3.27 file infobases and other configurations than БСП were not measured;
  the format version check accepts 8.3.8 files of any platform.
