# Design: schema-driven Form ListSettings tail

`ibcmd-schema` embeds and parses
`edt-2025.2.3-dcs-writer-evidence.json` under strict byte, collection, and text
bounds. Unknown fields, duplicate facts, unexpected facts, altered evidence
status, altered release, and any mismatch in the verified namespace, tail
order, writer operation, defaults, delegate, or null branch fail closed.

The public schema API exposes one immutable tail policy:

1. `itemsViewMode`, emitted with `writeEnumNotDefault` semantics and omitted
   only when the writer corpus exposes exactly `QUICK_ACCESS` and the verified
   Xcore feature exposes exactly `QuickAccess`; no naming convention converts
   one representation into the other;
2. `itemsUserSettingID`, emitted with `writeStringNotDefault` semantics and
   omitted for the empty string.

Any release, evidence-status, writer-constant, or lexical-default mismatch
between the two corpora fails closed. The policy copies its lexical default
only from verified feature semantics. The bundled cross-corpus policy is
initialized once.

`ibcmd-xml` consumes this policy and returns only child XML for those two
fields. The caller supplies the existing prefix and indentation. Text is
escaped by the XML layer. The emitter never creates a wrapper and has no API
for filter, order, conditional appearance, or opaque content.

`form_body` remains owner of `<ListSettings>`, its indentation, and every
preceding complex section. It delegates only the final two branches to
`ibcmd-xml`. The general DCS serialization preflight remains unchanged and
keeps the aggregate matrix of four missing facts. Each concrete diagnostic is
context-sensitive: it reports only its own standalone/Form wrapper fact, the
common type fact, and opaque placement only when opaque input is present.

Tail values must consist only of XML 1.0 characters. Prefixes are bounded
NCNames and reject the reserved `xml` and `xmlns` names case-insensitively.
