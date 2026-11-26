# Help index: saved wave2 structural survey for wave3

This is offline research, not a product codec or runtime help-search acceptance.
Input is the immutable wave2 native/own raw payload set already recorded in
`wave2/help-index-raw-audit.json`. Raw evidence and reproducible scripts are on F:
`F:/ibcmd/lab/05/wave3/coordinator/help-index-probe/`, scripts one directory above.
No database was accessed or changed by this survey.

Raw-deflate inflation is bounded to 8 MiB and rejects incomplete streams,
unconsumed input and trailing bytes. Docs and Vocabulary decode strictly as
UTF-16LE. Postings is binary, not UTF-16 text. All saved bytes are accounted for:

| Saved image | Docs records | Vocabulary terms | Postings document records |
|---|---:|---:|---:|
| own preimage | 628 | 12,308 | 133,371 |
| native first and second | 627 | 12,301 | 133,210 |

Docs records contain five tab-separated fields: numeric document ID, one numeric
field whose meaning is not established, a version fingerprint, document URL and
title. Vocabulary records contain a term, byte length and byte offset into
inflated Postings. Every vocabulary range is consecutive, disjoint and covers
the complete Postings payload exactly.

Postings integers use big-endian groups of seven bits with the high bit marking
the final byte. Each document record starts with document ID and the byte length
of its following position stream. That stream also parses into bounded integers;
its complete semantic interpretation is not claimed. Integers are bounded to
five bytes/u32, reject noncanonical leading zero groups and must terminate within
their declared range. Document IDs are strictly ordered per term and every ID
exists in the corresponding Docs image. These checks pass for all records of
both saved images. The position-stream length counts bytes, not occurrences.

Native removed precisely document 2442051, the help URL of CommonForm.
СвязанныеДокументы (UUID 35bc58df-8aff-48dd-83a9-b60b0461f94d). The remaining
627 Docs records are exact; no new Docs record appears. Filtering that ID from
each own Postings term, removing empty terms and recomputing Vocabulary offsets
reproduces the entire inflated native Vocabulary and Postings byte-for-byte.
This is stronger than comparing word lists or ignoring compression differences.
The reproduced SHA256s are the already recorded native inflated SHA256s.

The removed Docs fingerprint is the old form Help (.1) configVersion. All 628
preimage fingerprints match the preimage effective `versions` overlay: a version
UUID rendered as its little-endian bytes followed by `00000000`. Exactly one
preimage fingerprint becomes stale against either subsequent native overlay,
and it is the removed document. The exported help HTML itself has identical
SHA256 before and after. This supports a version-invalidation hypothesis even
when page content is unchanged; it does not establish a complete writer rule.

Before implementing publication, obtain fresh native controls for no change,
module-only change, help-version change, actual help-text change, nested owners,
languages and repeated pending generations. Retain all three index rows with
full physical headers and relevant effective version overlays. Determine whether
native rebuilds new documents eagerly or relies on the existing per-SHA Files
shards/lazy search; master-index invalidation alone does not prove search parity.
Measure the 8.5 layout independently. Unknown URL/layout/encoding/fingerprint
tails must refuse rather than be interpreted by analogy. Any eventual writer
must capture full Files preimages, participate in semantic/physical CAS, bounded
publication and recovery, and receive independent code/spec/raw-evidence review.

Reproduction: `help-index-probe.py`, `help-index-delta.py`,
`help-postings-survey.py`, `help-versions-survey.py` in the coordinator lab.
Outputs `survey.json`, `delta.json`, `postings-survey.json` and
`versions-survey.json` preserve raw provenance and exact checks. Earlier failed
UTF-16 Postings decoding remains valid historical evidence; it is not replaced
with an invented text encoding.
