# Design

Add ibcmd-edt as a format adapter over ibcmd-core and ibcmd-xml. Reuse the
smallest practical source-only morph1c XML codec closure in a pinned, attributed
vendor snapshot; exclude its CF backend, CLI, platform processes and unrelated
research tools. Its intermediate descriptor/body structures stay private.
Expose EDT project inventory and canonical metadata through existing ibcmd-core
and source-tree contracts. Keep dependency direction core <- codecs <- root.

Extend existing convert with explicit edt endpoints and EDT profiles. Profiles
declare the EDT project format, exact EDT evidence and associated XML dialect;
do not silently infer a runtime version or fall back to a latest/default format.
Generate valid DT-INF and src layout; preserve configuration UUIDs, ordered
properties, modules, forms and assets. Read and validate known EDT project-control
fields against the explicit profiles and UTF-8 serialization; account for those
fields as converted container controls. Unknown project-control fields fail
closed. Native XML does not carry EDT workspace/build settings, so this route
does not promise byte-identical EDT project-container control round trips.
Check that every input file is converted, reversibly retained, or rejected.
Unknown cross-format fragments fail closed with diagnostics. Read/resource
bounds, path collision/symlink guards, no overwrite, dry-run, preflight and
atomic publication apply to both directories.

Same-profile XML->EDT->XML retains the original tree with explicit provenance
where a format cannot represent lexical/storage details. The generated EDT
project must remain importable by EDT, and changed EDT objects must never be
masked by stale provenance. Provenance is validated against exact content hashes
and converted semantics. An EDT project without provenance uses typed codecs.

Oracle harness lives outside production and explicitly launches the installed
EDT on disposable F: workspaces. Capture native XML, EDT import/export XML and
our conversion, compare using existing source-three-way-oracle, and record exact
versions, commands, corpus hashes and complete verdicts. Large corpora remain
on F:. No writes to original research corpora or user databases.

The real EDT exporter omits ConfigDumpInfo.xml. Preserve full raw comparisons,
and separately compare configuration payload inventories with only that exact
storage/version manifest excluded, identifying its source hash explicitly.
All metadata/body files remain in the comparison. XML->EDT->XML must restore
the entire original XML tree, including ConfigDumpInfo.xml, byte-exactly.

Acceptance compares each route against the corresponding real exporter:
authentic EDT -> our XML against native ibcmd's export of a fresh database
loaded from that EDT export; original native XML -> our EDT -> installed EDT
XML against installed EDT's authentic import/export baseline. Both comparisons
retain complete raw three-way inventories. Differences between the two real
exporters are evidence, not a blanket normalization allowance. Fresh creation,
import and native export must succeed. Database activation is a separately
reported optional probe, not the import/export requirement of issue #356.

EDT import/parser failures block acceptance. Structured source validation is
compared against the authentic project as an exact diagnostic multiset with
only timestamps and workspace project labels excluded: conversion must add no
diagnostics. Existing business-code findings remain recorded and do not justify
claiming an error-free configuration. Ambient workspace diagnostics require
precise empty-project control evidence and a matching differential; unknown
or new errors remain failures.

Valid configurations must convert in both directions without arbitrary
configuration-size or XML-event-count exclusions. Streaming XML validation
shares the strict lexical/document parser but retains only ancestor state;
source-asset inventory and root dispatch must not build a body DOM. MXL SDK
projection processes qualified localized-content leaves in a single pass and
preserves other bytes, including binary payloads and structural whitespace.
Its iterative codec must not inherit recursive metadata depth/node ceilings.

Resource policy is distinct from format support. Complete source trees and
large assets need scalable storage and external digest/length references;
fixed in-memory source-file, inventory-count and aggregate-byte ceilings are
implementation gaps to remove, not acceptance exceptions. Ordinary bounded
reader policies and adversarial path/XML protections remain explicit. Raising
a magic constant does not complete the scalable-storage requirement. Preserve
atomic publication and stale-provenance protection when adding disk-backed
storage, and report actual resource exhaustion without classifying a valid
configuration as unsupported because of its size.

Assess useful morph1c knowledge for other milestones in a separate documented
matrix, without silently expanding this implementation's scope.
