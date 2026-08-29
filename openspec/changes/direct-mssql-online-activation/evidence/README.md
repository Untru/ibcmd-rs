# Activation protocol evidence

This directory contains bounded laboratory evidence for direct MSSQL
activation on 1C:Enterprise 8.3.27.2214.

Safety boundary:

- `BSP_Service` is read-only and is used only for a baseline snapshot.
- Oracle writes are allowed only in the disposable clone
  `ibcmd_rs_extlab_20260829`.
- Native `ibcmd` is an oracle in these experiments, not a runtime dependency.
- Direct online activation remains unsupported until an already-connected
  session observes a changed body without reconnecting.
