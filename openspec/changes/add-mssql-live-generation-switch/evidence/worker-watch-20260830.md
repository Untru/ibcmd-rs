# Dedicated-worker editor-watch evidence (8.3.27)

## Environment

- Platform: 8.3.27.2214
- Cluster/RAS: `localhost:2541` / `localhost:2545`
- Disposable MSSQL infobase: `ibcmd_rs_activation_main_20260829`
- Client PID retained: `26600`
- 1C session UUID retained: `d23755b9-8198-4896-a034-cc4cfef71e9b`

## Results

With an already staged generation, ordinary SQL promotion plus the worker
signal took 323 ms. The unchanged open client observed both new client and
server markers after 3,284 ms.

The complete `--mode worker --watch` save path was then exercised repeatedly:

| Save | Export | Classification | Staging | Activation | Total | Observed marker |
|---|---:|---:|---:|---:|---:|---|
| v6 | 400 ms | 32 ms | 1,167 ms | 5,788 ms | 7,510 ms | client-v6 / server-v6 |
| v7 | 433 ms | 16 ms | 1,173 ms | 7,279 ms | 8,997 ms | client-v7 / server-v7 |
| v8 | 734 ms | 17 ms | 1,651 ms | 5,955 ms | 8,525 ms | client-v8 / server-v8 |

The PID and 1C session UUID did not change. The database remained online; the
client reattached through a replacement `rphost`. The dominant and variable
cost is the platform worker handoff (4.3-6.2 seconds), not bounded compilation.

A shared-process preflight was also proved: it rejected foreign infobase
`be6fe5d3-3c29-4133-b337-19e1af5abd41` in 108 ms, and `ConfigSave` remained at
zero rows. A force-kill experiment was rejected by Windows access control and
was removed from the product; graceful `rac process turn-off` remains the only
supported signal.

## Cleanup

The test client and only the laboratory sessions were stopped. The original
SQL backup was restored with `CHECKSUM`; the database is `ONLINE/MULTI_USER`.
The registration has blank `db-user`, scheduled jobs are allowed, the temporary
SQL login is absent, and only the pre-existing 8.5 client PID 38544 remains.
