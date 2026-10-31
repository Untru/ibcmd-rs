# Design

The operator supplies one already-selected raw-deflate Form body file locally
inside the VM. It is never transferred to the development host. Bound compressed
and inflated bytes before calling the existing plain Form parser. Admit one root
at the first non-whitespace/optional BOM position and preflight depth/node work
against that same root before recursive parsing. Reuse shared raw attribute
views and existing slot accesses; admit all bag/query/collection bounds before
resolution, without running unrelated full attribute/DCS decoding. No new slot
guesses, callbacks or process-global state.

Reuse existing field-map, legacy twin, required-ID, shadowing and final-query
selection readers. Reports contain duplicate/required/order/equality facts only;
they do not assert why native omits a marker. Metadata-dependent universe results
are unavailable without the corresponding typed context. Wildcards, incomplete
or ambiguous evidence must not become false certainty.

Accumulate a bounded typed report and serialize once after success. The example
sanitizes input/parse/I/O/panic failures to static codes and prints no partial JSON.
No SQL connection, configuration edit, output export or acceptance normalization.
The library diagnostic is coupled to existing private parser code; the small
example wrapper adds no dependency or vendor adaptation.

Independent review and synthetic privacy/rejection tests precede operator use.
Its executable/source identity, input hash and bounds belong in operator receipts;
no private values are needed in the returned artifact.

## Optional frozen-input query identity context

A separate diagnostic mode accepts a bounded VM-local JSON context and both
frozen XML byte streams. The context binds their SHA256 digests and the compressed
body digest. Reviewed operator XML routines construct exact attribute-name and
canonical MainTable pairs locally; names and table references never leave the
VM and are not included in reports or Debug representations. This library checks
custody and exact unique raw-attribute joins; it does not independently parse or
attest the XML-derived context. Missing, duplicate or conflicting joins refuse.

The existing one-body API and version1 schema remain unchanged. Context mode uses
version2 and fixed public categories for source-match status, reference/deletion
identity spellings, alias classes and star scope. Unknown or ambiguous query
source identity leaves observation matrices null. Query absence/unparseability
is null. An unmatched lexical path never proves semantic nonselection.

The existing final-SELECT parser additionally retains plain-path occurrences for
this diagnostic, including duplicate selections. Its established paths map and
all production consumers remain unchanged. Expression counts preserve the boundary
of this lexical projection; this does not implement an SQL semantic resolver.

Context is at most 64 KiB, each XML at most 8 MiB, at most 64 named attributes; names
are at most 512 UTF-8 bytes and MainTable references 4096 bytes. All existing body,
query, collection and output-supervisor bounds still apply. No new dependency,
SQL call, XML export or production field-policy change is introduced.
