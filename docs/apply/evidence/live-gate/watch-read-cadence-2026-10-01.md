# Source watch read cadence (F14)

The watch loop polls file sizes and modification times every 100 ms. Unchanged
metadata reuses the last content digest until a one-second interval expires;
metadata changes trigger an immediate complete closure read. A mandatory
periodic complete read detects content changes even when both size and time
are preserved. Metadata is a hint, never proof that bytes are unchanged.

Only content-digest changes start the existing debounce. A failed metadata or
content read returns an error, preserves the last successful digest and forces
the next successful sample to read the whole closure. Source classification,
staging checks, activation policy and the worker lifecycle remain unchanged.
The one-second interval bounds the scheduling of full reads; it does not promise
completion within one second when storage itself is slow.

Deterministic regressions verify that nine unchanged 100-ms samples do not
reread contents, a same-size edit with its original modification time restored
is detected by the periodic read, and an inaccessible file cannot yield a new
digest until a successful full read after recovery. This is functional coverage,
not a native worker throughput or end-to-end latency measurement.
