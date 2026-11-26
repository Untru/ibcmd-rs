# Params `.ui` rows

The `.ui` rows of `Params` are platform licensing records; ibcmd-rs leaves them untouched. It does not read, create
or change them, and no publication mode writes them (`no_mode_touches_the_platform_licensing_rows_or_the_mobile_ring`
in `src/mssql_main_activation.rs`). A native `config apply` rewrites two of them and moves the `Modified` of a third on
every apply (`docs/apply/native-apply-trace.md`, 4.2). The 8.5 baseline took those rewrites for a generation switch:
"without them a fresh session does not see the new configuration". The measurements below do not support that.

## What a fresh session needs (measured 2026-09-29)

Method: a standalone server (`ibsrv` of the platform under test) is started on a lab database and answers a request
through a service of the configuration (8.5: the web service `InterfaceVersion`; 8.3.27: an HTTP service) with the
marker text of a common module. Every request is a new session. Databases: 8.5.1.1150, the БСП demo clone
`ibcmd_rs_04_ui_bsp85_s1`; 8.3.27.2214, a tiny database created natively (one common module, one HTTP service). The
module body was staged by our import (`mssql-stage-common-module`, five `ConfigSave` rows); the transitions are those
of `src/mssql_main_activation.rs` replayed in SQL. In no variant was a `.ui` row, another `Params` row, a `Files` row
or `ConfigCAS` written.

| variant | 8.5.1.1150 | 8.3.27.2214 |
|---|---|---|
| staged, not published | old module | not run |
| exclusive: each `ConfigSave` row replaces the `Config` row of its name, `ConfigSave` emptied; server started afterwards | new module | new module |
| exclusive for the two module rows only (`root`, `version`, `versions` unchanged); server started afterwards | new module | not run |
| online: `_dynupdate_<gen>` aliases, `versions_dynupdate_<gen>`, both `DynamicallyUpdated` markers; server started afterwards | new module | new module |
| exclusive, server already running | old module (14 sessions, 5 min) | not run |
| online, server already running | old module (24 sessions, 7 min) | old module (30 sessions, 4 min) |

Result: a session that starts after the publication needs the promoted `Config` rows and nothing else: not the `.ui`
rows, not `Files.MobileVersions.dat`, not the `DynamicallyUpdated` markers of the ordinary transition, not the new
`versions` generation for a changed body. A process that already runs keeps the generation it loaded and notices
neither transition on either platform. What a cluster worker does is not measured; it needs a cluster registration
of a lab database, and the service identity has no SQL access to the lab databases.

Warnings: in every variant the server's stderr was empty and its event log held information records only. The native
8.3.27 `config export` of the exclusively published tiny database ended with exit code 0, printed only its two progress
lines and wrote the new module text. A dialog that a client would show at start-up is not visible to this check.

A native `infobase create --import --apply` can report success and still leave `Config` without `root` and `version`;
a session on such a database fails (the server answers HTTP 503). Check that `root`, `version` and `versions` exist
after a create.

Lab kit (not in the repository), `F:\ibcmd\lab\04\ui-codec\tools`: `srv.ps1` (standalone server), `probe_http.py` and
`probe_hs.py` (fresh-session probes), `publish_exclusive.py` and `publish_online.py` (the two transitions replayed in
SQL), `rowsave.py` (save and restore the rows of one twin), `mk_tiny_probe.py` (the tiny configuration).
