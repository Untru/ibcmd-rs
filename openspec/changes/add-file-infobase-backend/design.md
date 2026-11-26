# Design: file infobase backend

## One format crate, two seams

`ibcmd-filedb` knows the file and nothing of configurations: it gives rows
(`ConfigRow`: name, part, dates, attributes, size, stored bytes) and takes a
whole table of them. Everything above stays where it is:

- Export: `mssql_dump::file_infobase::load` assembles the parts of `CONFIG`
  and `CONFIGSAVE` into `OfflineRows`, and `dump_config` activates them as
  it activates `--rows-dir`. The completed-stage test is the SQL one (a
  `versions` row, no `commit` or `*.new`, every staged row listed by the
  staged `versions`), so a file and a database publish the same
  configuration for the same rows.
- Import: `crate::file_infobase` builds the entries of the new configuration
  with code that already builds `.cf` files, then writes them. No SQL path of
  the import is touched; the SQL patch stage reads its target in eight
  places, and a file infobase gets the full stage the platform itself writes
  instead of an emulation of each read.

## Why a full stage

The platform's import into a file infobase writes every row to `CONFIGSAVE`
(9 939 rows for the БСП tree into the БСП infobase, `deleted` empty). Writing
the same shape needs no reasoning about how `config apply` merges a partial
stage, and the `cf load` route keeps the target's bytes for every object the
tree left alone — the property the SQL patch stage exists for.

## Writing

The writer computes every byte before the first write: names and keys are
checked, then the record, blob and index objects are rebuilt in memory and
written over the object's pages, growing it at the end of the file when it
must. An object never shrinks; what it no longer uses goes on its own free
chain, as the platform leaves `CONFIGSAVE` after an apply. The index is
rebuilt whole, half full, from keys whose encoding was matched against every
native key read. Atomicity is the file system's: the copy is renamed over
the data file once it has been read back and its index verified, and the
original stays as a hard link (`1Cv8.1CD.ibcmd-rs.bak`, the last one only).

## Collation

The keys of the configuration tables are sort keys under the infobase's
locale. Two locales are measured (`en_US_POSIX`, `ru_RU`); a write first
recomputes the keys of every configuration-table index of the infobase --
written by the platform -- and refuses unless they all match, so an
unmeasured collation, or a mistake in a measured one, cannot produce a stage
the platform cannot find.

## Locks

POSIX record locks on Unix (whole file: write for an import, read for an
export), sharing modes on Windows. A POSIX lock belongs to the process and
goes with any descriptor of the file it closes, so the writer never opens
the data file twice: the copy is read from the locked descriptor.
