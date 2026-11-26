# Extension schema updates with data preservation

## Why

Issue #582: existing extension activation supports code and resources but refuses additions of data-bearing objects. An update introducing own catalogs and information registers must become usable on the measured 8.5.1.1150 and 8.3.27.2214 profiles while retaining existing data.

## What changes

- Measure native extension schema publication and object creation on independent disposable SQL twins.
- Add a schema-aware extension update planner, compiler support and atomic publisher using existing S1/schema adapters where their measured rules agree.
- Hold unchanged main configuration, other extensions and stored records stable; validate new tables in a fresh platform session.
- Add generated Coverage regression cases, rollback, concurrency and idempotence evidence for both platforms. Unknown or destructive migrations are diagnosed before writes.
