# Design: offline three-way source oracle

The command receives three directory paths and four required provenance strings.
It scans files read-only, rejects output paths within an input tree, and refuses
to overwrite artifacts. Hashing is bounded by caller-configurable file count,
aggregate bytes, and per-file bytes. A tree hash is SHA-256 over sorted
`path + NUL + file SHA-256 + NUL + decimal size + LF` records.

Each input file is opened once. Its handle metadata is checked against the
per-file and remaining aggregate budgets before reading; two bounded hash passes
seek and read the same handle, then compare hash, size, modification time and
available file identity with the initially accounted state and the current path.
Symlinks, reparse points, replacement and growth fail closed.
On Windows, input handles deny delete sharing and identity is the stable volume
serial plus file index returned for the open handle; path rechecks open a second
handle and compare the same identity. On Unix, relative path components that are
not valid UTF-8 are rejected rather than lossy-normalized into colliding report
keys.

JSON and Markdown must share one existing canonical parent. Publication creates
two unique `create_new` temp files there, writes, flushes and `sync_all`s both,
then revalidates the parent path and identity. Atomic hard-link publication is
used as a no-overwrite primitive. Failure after publishing either final rolls
back every published final and temp. Markdown treats provenance and paths as
plain text with deterministic entity escaping for table/control characters.

For the union of paths, `Option<SHA-256>` values are compared directly. This
also deterministically handles one-sided paths. The classifier has five complete
branches: all equal; native=EDT!=ours; native=ours!=EDT; EDT=ours!=native; and
all different. Branch descriptions deliberately say *candidate*, because three
content hashes cannot establish a decoder/model/schema/writer, EDT, native,
storage, or version cause.

The EDT Convector inventory establishes that EDT import and export pass through
its model/writer stack, but no EDT bundle, Java source, proprietary binary, or
application export is committed here. The wrapper neither finds nor launches
EDT; EDT is an optional external producer of one immutable input tree.
