# Current form-module empty scaffold framing

The genuine EDT 2026.2.1.3 project in `issue357-current-server-dcs-002`
contains two identical form modules made only of empty Russian region sections.
Their UTF-8 EDT source is 471 bytes without a signature. The pre-fix reverse
converter wrote 474 bytes with a native signature; the independent native
8.5.1.1592 export is 492 bytes. Every difference is exactly one of 18 LF
separators expanded to CRLF. Final-newline presence and every other character
match. These public synthetic witnesses are retained under
`tests/fixtures/form-module-scaffold`; this is not BSP/UH or executable BSL
newline evidence.

A source-only form facet records an actual EDT module recognized as a complete
pure-LF empty region scaffold. Recognition consumes every current line, requires
balanced Russian region directives with ASCII/Cyrillic identifiers and at least
one pair, and accepts only ASCII space/tab framing. Designer emission rechecks
the entire current module before projecting only its LF separators to CRLF.
It never replaces a module with cached source or fixed fixture text.

EDT emission stays exact, and native-origin or manually constructed modules keep
the existing verbatim/BOM-only contract. Current edits adding executable code,
comments, literals, other directives, unbalanced regions, CR/mixed endings or
unfamiliar whitespace disable the projection and remain verbatim. Region-name
and blank-line edits use the current text, including its trailing newline count.
The module string, semantic digest, provenance checks and strict comparison
rules are unchanged; EOL-only text edits remain fingerprint-visible and forged
generated hashes cannot restore old XML.

The facet sits with existing source-only form sidecar framing fields in
`FormBody`, keeping the public `NamedFormBody` constructor shape stable. It is
populated only for that form's attached module, skipped by semantic serialization,
and revalidated against that same current module during writing.

This change is deliberately limited to the witnessed empty editor scaffold.
General executable BSL native-export equality and clean installed acceptance
remain unproved; no failed raw comparison is waived.

Focused checks include the actual directory encoder's complete source accounting,
publication without provenance, and a separate unchanged provenance return.
Directory stages and intermediate cleanup honor explicit
`IBCMD_TEST_KEEP_FIXTURES=1`; otherwise they are removed, in unit tests as in
production. The form-module whitespace regression removes its fixture after
the test unless that variable is set (docs/testing.md, "Temporary
directories"); no unguarded memory-API tests are selected for this run.
