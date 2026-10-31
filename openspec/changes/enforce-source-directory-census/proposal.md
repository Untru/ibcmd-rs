# Preserve directory membership in held-source revalidation

## Why
HeldSourceRoot captures file identities and contents but discards directory membership after spelling checks. An empty sibling directory can be added, parked outside the source, or renamed below traversal limits without changing any compared file. This violates the strict source census required by #382/#386. This is a source-derived gap, not an executed failure or a demonstrated compiled-content exploit.

## What changes
Retain a bounded detached directory census only for filesystem captures and compare it during final held-source revalidation. Use one canonical file/directory namespace and an independently reviewed anchored directory-witness API. Preserve legacy file-only inventories and semantic classifiers without treating them as filesystem proof. Add small public-source counterexamples before production changes; no target database, compiler, SQL, native platform or MCP enablement.
