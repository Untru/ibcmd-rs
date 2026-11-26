# Full physical activation capture, F8 checkpoint

The generic activation reader previously returned empty Creation/Modified and
Attributes=0, even when the database row contained other values. This checkpoint
keeps the actual SQL style-121 dates, attributes, name, part, size and bytes.
Both main activation and the already-bound extension-prefix reader use a
bounded header/digest pass, a server-filtered binary read and a second identical
full-header pass. Binary SHA and length must match the first pass: an A→B→A
change during capture cannot substitute B bytes into an A preimage.

The generic main renderer compares Creation/Modified/Attributes/DATALENGTH as
well as name/part/DataSize/SHA before any write, including exact ConfigSave,
selected Config and existing Config/Params markers. Published staged rows are
checked with the same full metadata. A header-only change affects the existing
recovery token. The JSON schema/token encoding is unchanged; this checkpoint
does not alter the validation of historical LIVE manifests.

Main row/count/total-byte budgets are unchanged. Extension capture retains its
caller's existing count/total-byte budgets; binary queries contain at most 64
bound row predicates, avoiding an expression spanning a large CAS namespace.
Offline mode refuses before the first SQL header request. Certificate policy
is carried through both SQL backends.

Five debug regressions cover exact shifted dates/nonzero flags, binary A→B→A,
header-only drift, refusal before binary materialization when a budget is
exceeded, a 70-row extension namespace split across bounded queries, and token/
transaction-guard binding of each mutable header field (multiple cases share
one regression). Initial quick testing retained two failures from old SQL-shape
assertions; the assertions now require the full header shape. Repaired quick:
fmt, physical-adapter policy, workspace clippy and library tests PASS,
3681 passed / 0 failed / 10 ignored. Raw logs:
F:/ibcmd/lab/05/wave3/coordinator/f8-quick-repair1/.

This is a source/regression checkpoint. A current matching-binary database
control, independent immutable review and the final combined gates follow.
F8 remains open: online recovery still lists ordinary rows that were retained,
there is no generic guarded alias/marker undo command, and bytes still use JSON
number arrays. Atomic file publication was accepted separately in wave2.
