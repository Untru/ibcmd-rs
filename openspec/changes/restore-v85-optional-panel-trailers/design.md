# Design

The existing trailer has a revision tuple and one retained member before
twelve appended logical members. Presence flags at three schema-owned
positions may each introduce one generated empty auto-command-bar record.
Public stored_panels_set.py constructs these records from a native empty
command bar, gives each its built-in identity and marks it generated. Native
load/save/load preserves the records; XML reconstruction removes them while
retaining identical native XML.

The parser enumerates only the four possible physical trailer lengths,
validates exact consumption and permits one unique boundary. All field
positions, accepted flags and generated-panel constraints belong to
ibcmd-form-schema. The adapter parses tuples and supplies their typed shape;
it does not scan arbitrary nested records for a convenient revision marker.

The converted legacy root retains the revision tuple at its true boundary.
The fact pass receives twelve logical members in their original order and a
separate classification of the known generated records. Unknown payloads or
ambiguous boundaries are errors. A declared revision-59 root cannot silently
use the generic legacy path after that error.

Searching for the last tuple beginning with 59 was rejected: unrelated data
can carry such a tuple. Keeping a fixed offset was rejected because each
presence record changes it. Adding guessed defaults was rejected because the
native table member is already known and must be reached through its profile.

This affects other simple properties on the same stored-panel profile. Their
native comparisons and prior baseline are part of the full corpus gate. No
whole-file acceptance is inferred from matching only the hierarchy property.
