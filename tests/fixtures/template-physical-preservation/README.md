# Native template patch preservation

These rows retain the actual native mechanical D2 stage and the failed OWN
stage before apply. The three descriptor companions have equal complete inflated
bytes but different raw DEFLATE encodings in the pre-fix OWN output. Native kept
the exact existing descriptor blobs. The native spreadsheet body and OWN body
differ by one leading language flag: native retained the existing `0`, while the
base-free writer emitted `1`. The template XML does not publish that flag.

`provenance.json` records each original F-drive artifact, byte count and SHA256.
The fixtures are independent of a database. The assertions compare entire
decoded bodies and exact raw descriptor blobs; they do not normalize metadata.
The actual OWN attempt refused before apply. These fixtures are codec evidence,
not evidence of OWN activation, session behavior or whole storage parity.
