# Managed worker authentication mutation controls

The internal creator previously challenged read-only administrator lists. A
read-only response does not prove that administrative writes require the
supplied credentials. The creator now verifies a correct register/present/remove
control, then challenges registration with wrong credentials and implicit OS
authentication separately for agent and cluster administration.

Every control and challenge uses a generated own administrator name. Complete
authenticated administrator records are compared before/after, including their
opaque fields. A known nonzero result with an observed mutation is a bypass,
and a successful result without a mutation is unclassified. Neither result
grants ownership. Only a completed challenge that adds exactly its own expected
record without changing the baseline may remove that record with correct
credentials. An unknown original process/pipe result stops without another
collector, removal, retry or cluster operation.

The native adapter checks the current private endpoint before every command and
retains the existing original-child execution/custody, deadline, profile and
pre-authentication worker guards. The challenge journal records families,
outcomes, lengths and hashes, never credentials or native output text. Denial
fingerprints are scoped to the administration family and challenge kind.

**Admission remains closed.** The reviewed native denial table is empty. No CLI
constructor is connected, and these changes do not accept managed Ready or
enable SQL writes. The pure protocol tests exercise both families with a test
adapter; their successful path does not populate the production denial table.

Historical laboratory AUTH v5 observed both correct controls and two agent
denials before its fixed measurement deadline. Cluster negative writers were
not started. AUTH v6 subsequently refused during startup, before RAC, with
original controller pipes unproved and worker custody retained. These partial
measurements supply protocol design evidence, not a full native authentication
certificate or proof that the changed Rust creator has run successfully.

Full native measurement, matching executable provenance, positive ownership,
pre-authentication worker history, CLI integration and operational recovery
remain part of issue #409.
