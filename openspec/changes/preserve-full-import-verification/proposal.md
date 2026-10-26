# Preserve supported source semantics through full-import verification

## Why

The final import guard rejects some XML spelling and ordering differences while several codecs lose supported form and metadata coordinates. These failures prevent a verified full-tree import even after the SQL Server 2012 transport and query fixes in #572. Transport compatibility remains a separate, explicitly selected major-version-11 capability.

## What changes

- Compare empty XML leaves consistently, decode XML references, and normalize only admitted metadata Type member sets and Rights object order without discarding values, qualifiers, rights or restrictions.
- Apply the existing conservative DCS namespace comparison at the final guard as well as the compiler boundary.
- Preserve measured form color, search width, CatalogRef.Parent and scoped UseAlways coordinates through typed codecs.
- Preserve nondefault Document tabular-section fill values and support a strictly validated existing external-source TableDataType edit.
- Retain refusal for unsupported source properties, unknown native profiles and every unclassified difference. Native EPF probes are diagnostic evidence, not permission to ignore properties or rewrite application XML.
