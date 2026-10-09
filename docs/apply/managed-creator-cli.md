# Fresh managed worker through the public MSSQL commands

`mssql-apply-source-change` and `mssql-activate-staged-main` select a new private
worker lifetime when `--managed-worker-parent` is present. The implementation
is Windows-only, for `platform-8.3.27.2214`, main configuration, `--mode worker`
and the built-in SQL client. `--allow-non-lab` is always required before creation.
The database must already exist; the command does not create or restore it.

Supply every member of the explicit creation group:

| Argument | Meaning |
|---|---|
| `--managed-worker-parent` | Existing absolute parent for a fresh random private root |
| `--managed-platform-bin` | Absolute directory of the selected 1C executables |
| `--managed-powershell` | Absolute PowerShell executable |
| `--managed-agent-port`, `--managed-cluster-port`, `--managed-ras-port` | Three nonzero, distinct private ports |
| `--managed-worker-first`, `--managed-worker-last` | Inclusive private worker range, 1 to 128 nonzero ports |
| `--managed-timeout-seconds` | Existing managed command/startup/shutdown ceiling, 1 to 120 seconds |
| `--managed-registration-user`, `--managed-registration-pwd-env` | SQL login and password environment slot for the new 1C registration |
| `--managed-infobase-user`, `--managed-infobase-pwd-env` | Explicit infobase login and password environment slot |

The three control ports must fall outside the worker range. The native creator
also checks private listener absence and pins the actual executable files.
The declared profile is checked before creation; the created endpoint's actual
platform profile and storage layout are verified before source staging or
activation SQL. Source apply validates its supported body path and selected
input files before creation, then repeats normal consumer checks.

`--server`, `--database`, `--sql-user` and `--sql-pwd-env` select the Rust SQL
client separately. This route requires an explicit nonempty SQL login and
password. SQL, registration and infobase passwords are resolved once, in
memory, before creation. A missing or empty secret is refused; an explicitly
empty `--sql-pwd` never falls back to another provider. No environment slot
name or secret value is printed by this orchestration layer. The existing
native RAC registration protocol passes credentials in its process arguments;
that inherited F-12 exposure remains a separate limitation.

For source apply, also supply `--source-root` and `--path`, using an existing
measured main-configuration module, form or template body. The same source
cohort check as ordinary source apply runs before creation; nested forms and
unmeasured roles remain refused. For already-staged activation,
use the same creation group without those two source arguments. The generated
cluster and infobase UUIDs, RAC path and RAS endpoint come solely from the
original Ready owner after registration/load. `--cluster-id`, `--infobase-id`,
`--rac`, `--ras-endpoint`, `--infobase-user` and `--infobase-pwd` conflict with
fresh creation. Existing-endpoint commands still require both UUIDs and retain
their ordinary worker ownership refusal.

`--tail-log-output` is also refused at entry to either fresh command, before filesystem
checks, credentials, creation, registration or loading; it is a live-only option.

`--watch`, extensions, live checkpoints, session interruption and external
`--sqlcmd`/`--bcp-executable` are refused before creator effects. A fresh managed
`--dry-run` is also refused before creator effects: use an existing verified
target for an ordinary dry-run. No watch loop reuses or recreates this owner.

The public call owns the creator result for its entire lifetime. A Ready owner
registers and loads exactly one target, then is borrowed by the existing
managed source/activation consumer. Publication success requires the actual
typed `Committed` state and proved replacement handoff before report
serialization. The public call returns a successful report only after the
same owner proves original process completion, both pipes, its exclusive
journal and private process/listener absence through owned cold shutdown.
A no-op source or activation still proves that cold shutdown before returning.

If creation returns `Retained`, or registration, source staging, publication,
handoff or cold proof fails, the command prints a nonsecret `managed UNKNOWN`
location/phase diagnostic and stays resident. It keeps the original runtime,
process and reader references, kernel handles, exclusive journal, original
error/panic object and any already-completed consumer report. It does not
return an ordinary error that drops those owners. It never automatically
retries, adopts PIDs, signals on failure, reconstructs ownership from JSON or
runs SQL undo. A confirmed commit with an unproved handoff is UNKNOWN, not a
successful report. Waking the resident thread does not permit another attempt;
external termination is an explicit operator decision and is not cleanup
proof. The retained private root/journal are not automatically deleted.

The new consumer tests cover public CLI selection/refusals, separate credential
resolution, generated target mapping, typed outcome checks, same-owner order
and retained failures with mocked effect leaves. They do not establish native
process, SQL, handoff or cold-shutdown acceptance. Compilation, test execution,
independent Source review and actual native acceptance remain separate gates.
The route depends on the managed native creator/utility implementation from
PR #459; this document is not evidence that that dependency or runtime is accepted.
The managed group is all-or-nothing: all thirteen fields, including parent, must
be supplied together. Every managed field conflicts with the six old target
UUID/endpoint/infobase credential flags. Partial programmatic arguments are also
refused before source/credential/creator effects in apply, activation and watch.
An empty group keeps the existing-target route and its existing policy checks.
