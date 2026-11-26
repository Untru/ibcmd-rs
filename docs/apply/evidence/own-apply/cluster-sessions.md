# New sessions see the applied change: the 8.3.27 cluster (issue #337)

2026-09-29, БСП 8.3.27.2214, SQL Server, cluster ragent `localhost:2540` (clients `localhost:2541`), the lab clone
`ibcmd_rs_04_apply_bsp8327_probe_20260929` registered with `tools/register-ib.ps1` (the script reads the SQL login
itself; scheduled jobs denied) and unregistered afterwards. Sessions are external connections through the 64-bit COM
connector (`V83.COMConnector` resolves to the 8.3.27.2214 `comcntr.dll`) as the БСП user `Администратор`; each reads
`СтроковыеФункцииКлиентСервер.ПробаПримененияIbcmdRs()`, a function of a common module with the server and external
connection flags. The change is the returned string, staged by `ibcmd-rs infobase config import` (patch mode, 9 517
rows, 9 515 identical: the module row and `versions`) and applied by `ibcmd-rs mssql-config-apply`.

| when | session | reads |
|---|---|---|
| before the first apply | a new session | `ibcmd-rs-apply-probe-v1` |
| after the first apply (SQL-proved exclusivity: the baseline session had closed and left no SQL connection) | a new session | `ibcmd-rs-cluster-probe-v1` |
| warm session W opened, then a second change staged | W, before the second apply | `ibcmd-rs-cluster-probe-v1` |
| the second apply with the default exclusivity | (refused) | "exclusive access is not established: 3 other session(s)" -- three connections of program `1CV83 Server`, listed by session id |
| the second apply with `--exclusivity assumed` (the operator knows the only session is idle) | new session N2, opened after it, while W is still open | `ibcmd-rs-cluster-probe-v2` |
| the same | W, 30 s after the apply | `ibcmd-rs-cluster-probe-v1` |

Result for #337: **a new session in the cluster sees the applied change**, also in a working process that still
serves a session opened before it. For 0.5 (information, not a 0.4 requirement): a session that was open before the
apply keeps the configuration it started with -- it read the old value 30 s after the apply and gets no notification;
only a new session sees the change. The cluster's working process opens three SQL connections per registered
infobase in use (program `1CV83 Server`, login of the cluster's SQL account) and drops them when the last session
closes, so the SQL exclusivity check passes once every session is closed.

Repeat: `tools\cluster_probe.ps1` (lab; `-Mode read`, or `-Mode hold` with a ready file and a go file).
