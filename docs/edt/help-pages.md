EDT help markers carry an ordered list of language identifiers. Each `<pages>` item
contains one `<lang>` value; every identifier is bound to an owned HTML page. The
reader rejects missing, extra, duplicate, or malformed declarations. Native
`Help.xml` page order and EDT marker order become the same typed list, including
help attached to ordinary and managed form references.

Writing derives marker identifiers from the current `HelpPage` records. It uses a
descriptor view that excludes module, form, and binary body copies; the sidecar
writers still receive the complete original object. Changing a page language or
order updates the descriptor and output filenames. Resources remain byte exact.
Language identifiers have filesystem component safety checks and no arbitrary
byte-length or ASCII-only restriction.

Historical verification for this change is recorded on F: in
`lab/07/multilingual-help-research-r1` and `multilingual-help-tests-final-r1.log`.
The installed EDT model exposes an ordered `Help.pages` list. The standalone
`loader-r12` probe loads and saves two element-form pages and verifies complete
EObject equality, both language values, and their order. Original installed class
bytes and dependencies are hash-bound; its lab loader and ordinary serializer
dependencies are recorded explicitly. This model/serializer probe is separate
from headless project import/export acceptance.

Public conversion controls verify exact unchanged native source return, conversion
with provenance removed, and refusal to replay an old source after a language and
filename edit even when all generated hashes are replaced. Focused controls also
cover Unicode identifiers, XML escaping, both form types, resources, and malformed
declarations. The genuine regression retains the prior RU-only Configuration and
CommonCommand descriptor witnesses from both laboratory versions; it is not a
claim that complete corpus acceptance has passed.
