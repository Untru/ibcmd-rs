# Online recovery preimages and source path consistency

F8 previously labeled every selected ordinary Config row as overwritten. An
online publication replaces only root/version, inserts new aliases beside the
other ordinary rows, and updates the two history markers. The recovery snapshot
now keeps root/version in `overwritten_config_rows` and the other exact captured
preimages in `retained_config_rows`. A no-op has no overwritten Config rows.
Retained preimages remain part of the recovery token and publication CAS; they
must not be restored over a later ordinary configuration by an operator.

The online report names the new aliases and both fully qualified markers, so
its nested activation result describes the row changes as well as the touched
tables (F13). No-op and ordinary modes claim no new dynamic aliases/markers.

The retained field has serde default and is omitted when empty. An explicit
historical six-field serializer regression verifies byte-for-byte unchanged
non-no-op LIVE token input and deserialization of old snapshots. This does not
convert historical online snapshots into a guarded undo format. A generic
alias/marker undo command and compact binary serialization remain open.

F14 source inventory already used a Unicode lowercase key, while canonical
root equality and containment used ASCII comparison. Both now use the same key
as source inventory and lexical paths. Containment still compares complete
path components, refuses adjacent prefix roots and requires Unicode paths.
This aligns the existing project policy; it does not introduce a new filesystem
normalization policy or change the watch loop's hashing cadence.

Regressions cover exact online retained/overwritten sets and alias/marker names,
no-op claims, historical LIVE serialization/token compatibility, and Cyrillic
case variants across inventory/canonical-root/component containment. Existing
header drift/token checks continue to cover retained ordinary preimages.
Focused recovery checks:19 passed/0 failed. Mandatory quick gates and immutable
independent review are recorded by the coordinator before publication.
