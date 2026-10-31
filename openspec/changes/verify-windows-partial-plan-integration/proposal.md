# Verify Windows FileDbPlan partial staging integration

## Why
The Windows target snapshot/publication controls in #530 and source-custody controls in #533 are independently exercised, but the editor partial-import integration target is entirely cfg(unix). Its Windows zero-test result does not establish that actual FileDbPlan preparation, compiler verification, retained source/target binding and publication work together (#382).

## What changes
Add only tests/editor_partial_import_windows.rs and this test contract/CI selection proposal on a separate ordinary local merge stack of the immutable reviewed donors. Exercise real Plan APIs with public synthetic fixtures, fresh child per compiler stage, exact CONFIG/backup and complete CONFIGSAVE/effective-state oracles plus refusal/recovery cases. Do not change production, donor histories, Unix tests, OS guards, import planning, server/MCP or engine observers. No native1C, SQL, apply or persistent-server acceptance is inferred.
