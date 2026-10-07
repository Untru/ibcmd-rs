# Worker cluster lab contexts

The existing default and wave1 contexts retain their historical implementation.
The following fresh wave3 contexts use a separate guarded lifecycle:

| `IBCMD_RS_WORKER_LAB_ROOT` | Track | Allowed restored database names |
| --- | --- | --- |
| `F:\ibcmd\lab\05\wave3\load\cluster` | `load` | `ibcmd_rs_05_load_w3_*` |
| `F:\ibcmd\lab\05\wave3\metadata\cluster` | `meta` | `ibcmd_rs_05_meta_w3_*` |

These are exact mappings, not arbitrary directory overrides. Both use platform
8.3.27.2214, agent 5540, manager 5541, RAS 5545 and worker ports 5560–5591.
Hold the matching shared worker FIFO lease for the **entire** lifecycle. Native
commands additionally require their individual native FIFO holds. No Windows
service or existing cluster settings are changed.

`start.ps1 -Track load` (or `meta`) refuses existing state, existing `srvinfo`,
foreign listeners and unknown processes bearing these root/port arguments
before starting anything. It records each directly spawned process, then
retains the union of discovered descendants and their ancestry. Identity is
PID, UTC creation ticks, exact executable and exact command. Ports only identify
conflicts. A fresh empty registry may receive one explicit `rac cluster insert`
after bounded successful RAS inventory and exact version verification; unknown
inventory refuses this bootstrap. Failed startup retains ownership state.
During fresh startup only, listener acquisition uses a full process census,
then listeners, then a second full process census within the original startup
deadline. Each positive listener must have the same PID, UTC birthday,
executable, command and parent in both censuses, with proven owned ancestry in
both. Missing or vanished observations are sampled again without starting or
signalling anything; foreign, non-whitelisted or changed identities refuse
immediately. The measured C1 listener PID absent from the historical census
remains unproved; the new acquisition never retroactively admits that PID.
Outside startup, including stop, the immediate listeners-first/single-census
refusal guard remains unchanged. Failed listeners retain a bounded sanitized
receipt from the already captured censuses, including an observed listener
executable outside the server whitelist. Ports and counters are not ownership.
Startup refusal receipts also retain the original absolute deadline, its
initialization time, acquisition entry/observation time and current port/RAC
phase. Expiry has reason `startup_deadline_expired` and no failed-listener PID;
the preserved censuses may contain entirely owned listeners. It never admits
an observation at or after the deadline. Observed foreign listeners and identity
drift still refuse immediately with their distinct reasons. The historical D2
60-second refusal did not record the absolute deadline; its owned two-census
replay supports expiry, without proving the unavailable historical clock value.

Use `private-register.ps1 -Action register -Database <owned-new-clone>` and
`-Action unregister` for these contexts. The wrapper binds the current cluster
and RAS listener before invoking the shared registration helper. Every registered
name, including disconnected registrations, must match the selected track,
prefix and shared restore manifest. Recorded backup origins must resolve beneath
the shared F backup directory or the corresponding wave3 lab directory. Prefix
alone never authorizes a registration or process signal. A failed/timed-out helper
may have changed registration; inspect retained logs before retrying.

Before `stop.ps1`, close the exact owned client sessions and unregister all
infobases. Stop refuses missing/corrupt state, changed lease/identity, foreign
listeners, unknown registration or any remaining registered infobase. It retains
descendants before each signal, stops the spawning agent first and continues
using saved identities even if anchors exit. An unavailable RAS inventory refuses
cleanup rather than assuming no registrations. Failure preserves state.

Ordinary stop preserves `srvinfo` and logs, and archives state only after all
owned processes and listeners are gone. For `stop.ps1 -Purge`, preserve evidence
outside the selected cluster root first. Both final deletion targets and every
contained/ancestor reparse point are checked before either recursive deletion.
Existing registry history cannot be reused for another fresh start.

`test_private_ownership.ps1` exercises identity, orphan retention, PID reuse,
foreign executable/command/listener, registration, FIFO, path/reparse and actual
entrypoint refusals with mocked OS calls. It starts/signals/deletes nothing.
`test_private_startup_deadline.ps1` runs the actual functions with only a mock
clock and in-memory observations to check exact deadline, foreign and identity
boundaries without writing receipts or acquiring resources.
These mocks establish refusal boundaries; native lifecycle acceptance is a
separate measurement. The kit covers these fresh, recorded lifetimes only.
General historical/unregistered loaded-infobase ownership (F11) remains open.
