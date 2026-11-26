# Isolated platform 8.5 wave3 lab cluster

This experimental kit owns only the fresh exact-8.5.1.1150 private cluster at
`F:/ibcmd/lab/05/wave3/platform85/cluster`, ports 6540/6541/6545 and 6560–6591.
It does not change the shared 8.3 worker or platform capabilities.

Acquire the shared worker FIFO before starting the cluster. A fresh data folder,
absent ownership state and free private ports are required. Registrations must
use the shared `register-ib.ps1 -Cluster p85worker -Platform 8.5 -Track p85` helper.
Only new manifest-owned `ibcmd_rs_05_p85_w3_*` databases may be registered.
Remove registrations before `stop.ps1`, then release the worker FIFO. Preserve
the previous data folder and stopped-state evidence before another fresh run.

Cleanup retains the union of every discovered descendant identity (PID, UTC
creation time, exact executable and full command). It remembers descendants
before signalling the spawning agent, then checks retained identities directly
even after their parent has exited. PID reuse or identity drift refuses cleanup;
a port or executable name alone never authorizes a signal. Failed cleanup keeps
the live state for inspection/retry. Root/state/data reparse points are refused.

The pre-signal registry snapshot includes disconnected registrations. Its
manifest-owned superset is useful only for this fresh isolated cluster with a
controlled registration lifetime. It is not a general per-process loaded-IB
inventory, and cannot prove that an arbitrary historical cluster has no
unregistered but loaded infobases. General product WORKER/F11 admission remains
separate and fail-closed.

`test_ownership.ps1` exercises descendant/orphan identity retention, foreign
exclusion, UTC serialization, PID/command/executable drift, manifest names and
root/reparse refusal with mocks; it never starts or signals a process.

Observer launch and direct dialog priming validate their lab paths before any
read/write or UI action. A fresh label must have no journal, PID, identity or
client-output artifact, including a failed launch that produced no journal.
The command binds the exact private endpoint, manifest-owned database, EPF and
label. Observer timeouts are bounded; stop labels cannot contain wildcards.
