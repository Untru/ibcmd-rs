## ADDED Requirements

### Requirement: The export reads a file infobase without the platform

`infobase config export --db-path=<dir>` SHALL read `1Cv8.1CD` in the
directory and SHALL publish the same configuration the export publishes for a
SQL Server database holding the same rows: `CONFIG`, with a completed import
stage of `CONFIGSAVE` in place of the rows of the same names.

#### Scenario: A file infobase with a configuration

- **GIVEN** a file infobase the platform 8.5.1.1150 created from the БСП 3.2 demo configuration
- **WHEN** ibcmd-rs exports it with `--db-path`
- **THEN** every file it writes is identical to the platform's export of the same file

#### Scenario: A completed import stage

- **GIVEN** a file infobase whose `CONFIGSAVE` holds a `versions` row and no `commit` or `*.new` row
- **WHEN** ibcmd-rs exports it
- **THEN** it publishes the staged rows in place of the `CONFIG` rows of the same names

#### Scenario: A file that is not a 1Cv8.1CD of format 8.3.8

- **GIVEN** a truncated file, or a file without the `1CDBMSV8` signature
- **WHEN** ibcmd-rs exports it
- **THEN** it fails with a message and writes no tree

### Requirement: The import stages the whole configuration in CONFIGSAVE

`infobase config import --db-path=<dir> <tree>` SHALL replace every row of
`CONFIGSAVE` with the rows of the tree's configuration, `root`, `version`,
`versions` and a `deleted` row naming the `CONFIG` rows the configuration no
longer has, so that the platform's `config apply` takes the tree.

#### Scenario: An empty infobase

- **GIVEN** a file infobase whose `CONFIG` holds no row of the tree's configuration
- **WHEN** ibcmd-rs imports the tree and the platform applies and exports it
- **THEN** the platform's export is identical to the tree but for `ConfigDumpInfo.xml`

#### Scenario: The same configuration, edited

- **GIVEN** a file infobase holding the configuration of the tree, and a tree with an edited module and an edited property
- **WHEN** ibcmd-rs imports the tree and the platform applies and exports it
- **THEN** the platform's export is identical to the tree but for `ConfigDumpInfo.xml`
- **AND** every object the tree did not change is staged with the infobase's own bytes

#### Scenario: A stage that would not export back to the tree

- **GIVEN** a tree whose compiled rows export to something else
- **WHEN** ibcmd-rs imports it without `--no-verify`
- **THEN** it refuses, naming the files, and the data file is unchanged

### Requirement: A write is atomic, backed up and exclusive

The import SHALL write in a copy of the data file, SHALL read the copy back
and compare every row and the index with what it wrote, SHALL keep the
original as `1Cv8.1CD.ibcmd-rs.bak`, and SHALL put the copy in place by a
rename. It SHALL refuse an infobase another process has open before doing
any work, and SHALL keep the platform from opening the infobase while it
writes.

#### Scenario: An infobase a standalone server has open

- **GIVEN** a file infobase `ibsrv` serves
- **WHEN** ibcmd-rs imports a tree into it
- **THEN** it refuses at once, saying the infobase is open
- **AND** the directory holds no backup and no work file

#### Scenario: An infobase in an unmeasured locale

- **GIVEN** a file infobase created with `--locale=de_DE`
- **WHEN** ibcmd-rs imports a tree into it
- **THEN** it refuses, naming the locale, and the data file is unchanged

#### Scenario: A Russian infobase

- **GIVEN** a file infobase created with `--locale=ru_RU`
- **WHEN** ibcmd-rs imports a tree and the platform applies and exports it
- **THEN** the platform's export is identical to the tree but for `ConfigDumpInfo.xml`

#### Scenario: A row the index cannot key

- **GIVEN** a row whose name holds a character outside the evidenced collation range
- **WHEN** the writer is asked to write it
- **THEN** it refuses before the first write and the data file is unchanged

### Requirement: The drop-in syntax takes --db-path

The drop-in command line SHALL accept `--db-path` and `--database-path` for
`config export` and `config import`, SHALL refuse them together with
`--dbms`, `--db-server` or `--db-name`, and SHALL refuse `config apply`,
`config save` and `config import files` of a file infobase with a message
naming what is supported.

#### Scenario: A DBMS option beside --db-path

- **WHEN** a command line gives `--db-path` and `--dbms=MSSQLServer`
- **THEN** it is refused as two options that exclude each other

#### Scenario: Apply of a file infobase

- **WHEN** a command line runs `config apply --db-path=<dir>`
- **THEN** it is refused, the message saying that only export and import run on a file infobase
