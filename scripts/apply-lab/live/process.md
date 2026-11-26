# Bounded LIVE lab children

`Invoke-LiveBounded` keeps its `Executable`, `Arguments`, `TimeoutSeconds` API
and successful `{ ExitCode, Stdout, Stderr }` result, including rapid exits and
nonzero exit codes. It captures the direct live child's PID, UTC birthday,
executable, command and parent before granting timeout signalling authority.
A missing or changed identity refuses any signal. An admitted timeout signals
only the original process handle; descendants are never signalled. Reap and
output-pipe waits are bounded to5000ms each.

Process or output-pipe timeouts retain an unresolved lifetime state even after the direct handle exits:
that exit does not establish whether a wrapper left running descendants.
`LIVE_CHILD_EXECUTION_UNCERTAIN` includes sanitized identity and a command/argument
hash, without command text. An unresolved direct child also sets the sticky
`Test-LiveUncertainChild` predicate. A later successful command does not clear it.

Case4 optionally sets `IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT` to the exact fresh
`F:\ibcmd\lab\05\wave3\load\private4-child-receipts` directory. The separate metadata split controller may use only the exact fresh
`F:\ibcmd\lab\05\wave3\metadata\body2-child-receipts` directory. It must
pin its own dependency hashes and retain native/worker leases on any receipt
before and after cleanup, including immediately before release. This mapping
does not authorize a metadata lifecycle. Other roots,
relative paths and reparse ancestry refuse before launching a child. Immutable
bounded receipts propagate unresolved state across nested script processes.
Any receipt keeps the lifecycle pending; ordinary empty RAS/SQL inventories do
not authorize unregister, cluster stop, native FIFO or worker lease release.
An explicit subsequent owned-child/descendant exit or cleanup proof is required.
No automatic rebind, retry or deletion of these receipts is provided.

`test_process.ps1` uses OS mocks only. It checks normal/nonzero/rapid results,
missing initial identity, five identity drift cases, exact direct-only timeout,
unreaped-child state and bounded output pipes. These tests are preparation,
not native/runtime workload acceptance.
