# Generic WORKER ownership refusal

Generic `--mode worker` execution now refuses before staging or recovery/script
publication. A current RAC connection list cannot establish complete ownership
of an idle loaded infobase or the process's registration history. No RAC utility
is started by worker preparation or switching, and no worker signal is sent.
Fresh private-cluster lab ownership does not authorize the generic product path.

Source apply retains read-only profile verification, bounded active export and
source classification. An actually unchanged source returns its no-op report;
a changed source receives the ownership refusal before staging or publication.
Standalone activation retains read-only planning and allows a guarded no-op
stage cleanup. Dry runs and the other activation modes keep their behavior.
Worker watch refuses its unsupported execution capability at startup.

The defensive switch refuses even when called with an old prepared plan. Its
error states that no worker was turned off and asks the caller to inspect
retained recovery if activation SQL already committed; it does not claim rollback.

Regression coverage includes actual identical/changed BSL source classification,
dispatch ordering before staging/artifacts, the mode/dry-run/no-op combinations,
and preparation/switching with a nonexistent RAC executable. These are refusal
proofs, not loaded-session or complete process-ownership acceptance.

Raw local quick and all-targets logs are retained under
`F:/ibcmd/lab/05/wave3/load/f11-quick-v2` and
`F:/ibcmd/lab/05/wave3/load/logs/f11-v2-all-targets-check.json`.
The first driver attempt used Windows' WSL launcher instead of Git Bash and
failed before any build; its diagnostic and FIFO release are retained separately.

The prior worker experiment in `online-activation.md` remains historical evidence,
not current authorization for generic worker turn-off. Positive generic WORKER
support requires a separately reviewed loaded/history ownership authority.
