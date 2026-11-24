# Repeated publication and the observer session

[The initial CommonModule control](initial-common-module-2026-10-01.md) on
8.5.1.1150 recorded an old A/A client and a fresh B/B client. The journal
writer cached the infobase session number when the form opened. Reading that
field on later lines cannot independently identify the session serving a poll.

Build the live observer from the current source with
`scripts/apply-lab/live/build_observer.ps1`. Each successful poll now reads
the infobase session number in the same server call as the primary server
marker. The journal keeps its seven columns. Successful paired reads carry
`sid-source=marker-call` in the details column. A failed call writes an unknown
session and an error marker.

For a repeat measurement, open one poll-mode observer on the owned database
at A. Independently record the start and end of the B and C apply commands,
their exit codes and the physical generation identifiers. Collect several polls
before B, between B and C, and after C. Keep the RAC session/infobase UUIDs and
the client process identity over both changes to distinguish a restart or a
recycled session number. Keep the fresh-client controls in separate journals.

Pass the four command timestamps as UTC milliseconds since 0001-01-01, in
the same clock domain as the observer. From the repository root run

```text
python scripts/apply-lab/live/repeated_session.py JOURNAL B_START B_END C_START C_END
```

The result lists every observed client/server marker pair and its count per
interval, the session number and the SHA-256 of the input journal. A poll
spanning a publication boundary contributes to no interval. The analyzer
requires a complete poll in each interval and checks every record for a
reopen, session change, missing paired read or invalid event. Interpret the
pairs alongside the actual publications. An A/A result after C can be a valid
observation of an old client.

## Windows preflight on 2026-11-24

The product versions of ibcmd, ragent, rmngr, rphost, ras, rac, Designer and
the thin client were 8.5.1.1529. A private cluster answered RAC on 26465
with cluster port 26461 and an empty registration list. After shutdown the
private ports had zero listeners. A second private cluster ran under a dedicated Windows virtual service
account. RAC registered a disposable MSSQL database. Its SQL login had
db_datareader, db_datawriter, db_ddladmin and VIEW DEFINITION in that
database. An impersonation check returned 0 for sysadmin, db_owner,
CREATE ANY DATABASE and the count of other accessible user databases.

On this build CREATEINFOBASE returned 0 for a disposable file database.
The Designer EPF build returned 1 and reported no license. Designer on the
private SQL cluster returned 1 and reported the absence of a server license.
A separate file client attempt using a repacked EPF and a synthetic marker
configuration reported an invalid configuration signature. No poll journal
was produced. The three journal-analysis unit tests use synthetic data.

## Native file database control on 8.5.1.1529

An uncompressed copy of the repository's Windows 8.5.1.1529
`tests/fixtures/file-infobase/default-button.1CD.gz` supplied the active module
returning 42. In a private copy of `tiny-source` the only BSL edit changed
`Возврат 42;` to `Возврат 43;` and then `Возврат 44;`. For each edit run the
installed 8.5.1.1529 ibcmd against the **same file database**, with private
`--data` and `--temp` directories. The relative module path is
`CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl`.

```text
ibcmd infobase config import files --db-path=DB --base-dir=SOURCE --partial MODULE
ibcmd infobase config apply --db-path=DB --force --dynamic=force
ibcmd infobase config export --db-path=DB UNIQUE_EXPORT_DIR
```

All six commands returned 0. Native exports contained the expected complete
module text after each apply. Closed-file snapshots read with `ibcmd-filedb`
gave the following physical row counts.

| Step | CONFIG | CONFIGSAVE | CONFIG.DynamicallyUpdated |
| --- | ---: | ---: | --- |
| Baseline | 9 | 0 | absent |
| Stage 43 | 9 | 5 | absent |
| Publish 43 | 13 | 0 | first UUID |
| Stage 44 | 13 | 5 | first UUID |
| Publish 44 | 16 | 0 | first UUID, second UUID |

Each force published three aliases. Their descriptor, module and versions
payloads matched the corresponding staged payload hashes. At the second
publication all three first-generation aliases retained their hashes. The
ordered UUID list in CONFIG matched the two native command results after
decoding the GUID byte order. The [run record](repeated-file-1529.json)
contains the binary hash, command times, generation IDs, alias hashes,
snapshot hashes and hashes of the exported modules. The file control had
no active clients. It measures native storage independently of a SQL or
OWN writer and supplies no client visibility observation.

## Native SQL Server control on 8.5.1.1529

The starting marker configuration used a private file database. Native
`infobase dump` produced a DT. Native `infobase restore --force` loaded it
into a precreated disposable SQL Server database. A native XML export from
SQL confirmed the primary CommonModule contained A346 before publication.
Restoring the DT required a temporary database-level ALTER grant. After
restore, a permission check confirmed its revocation. The publications and
SQL snapshots ran as the private cluster's Windows virtual service account.

On the same database, change only the primary module's returned marker
to B346 and then C346. For each change use the same partial import, dynamic
force and XML export commands as the file control, with MSSQLServer
connection options in place of the file path. All six commands returned 0.
Both SQL exports contained the expected complete module text.

| SQL observation | Baseline | After B346 | After C346 |
| --- | ---: | ---: | ---: |
| Config rows | 12 | 16 | 19 |
| ConfigSave rows after apply | 0 | 0 | 0 |
| UUIDs in Config.DynamicallyUpdated | absent | 1 | 2 |
| UUIDs in Params.DynamicallyUpdated | absent | 2 | 3 |

Each import staged five rows. Each force published three aliases whose
payload hashes matched the staged descriptor, module and versions rows.
The second force preserved the three first-generation aliases.
Config listed the native publication UUIDs in order. Params listed an
additional leading UUID, identical after both publications. The
[SQL run record](repeated-sql-1529.json) includes both lists, command times,
hashes and row counts.

The thin client launched against the registered SQL base exited with code
1 during license checking, before writing a poll journal. The SQL result
covers two native storage publications. Same-session visibility and a
comparison with the OWN writer on its supported build remain open.
