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

The measured UH native source has 140,709 files, 130,638 directories, 9.59 GB
of bytes and a 127.57 MB maximum file. Source inventories have independent hard
bounds of 524,288 files/directories, 256 MiB per file and 32 GiB aggregate
(including reversible source preservation). Ordinary reader defaults stay at
65,536 files/directories, 32 MiB per file and 256 MiB total. EDT routes opt into
the larger explicit bounds. Inline canonical assets remain capped at 32 MiB;
external digest/length references may identify source files up to 256 MiB.

Assess useful morph1c knowledge for other milestones in a separate documented
matrix, without silently expanding this implementation's scope.
