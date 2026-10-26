# Design

The native SQL backend owns a small encryption-policy enum. TdsPool parses IBCMD_RS_MSSQL_COMPATIBILITY and IBCMD_RS_MSSQL_ENCRYPTION once when constructed, before opening any connection. Compatibility defaults off; its only explicit value is 2012. Empty, unknown and non-Unicode selectors fail before connecting. Public SqlTarget and SqlOptions layouts remain unchanged. Pooled and dedicated connections use that immutable selection. The public standalone connection API resolves the same policy. There is no automatic fallback after a TLS failure.

In compatibility mode, the first query after connecting reads SERVERPROPERTY(ProductVersion), accepts exactly four numeric components with major 11, and rejects NULL, malformed values and any other major before session SET statements, database-name checks or user commands. Database compatibility level is not used. Default mode preserves the original startup sequence.

The supported encryption settings are required (default), login-only and disabled. The last setting advertises no TLS support and therefore also sends login data without TLS; it is an explicit compatibility choice, not the default. Invalid or non-Unicode settings fail before connecting and do not echo the supplied value.

Non-required encryption is rejected without the explicit 2012 selector.

SqlClient exposes a default-false uses_mssql_2012_compatibility predicate; the native implementation delegates to the frozen pool policy. Only when this predicate is true, import_target_state reads two COUNT_BIG values, storage_table_stats reads table name/count/bytes/nullable checksum, and dynamic_update_rows reads the stored names. All other Client implementations and the external Tools branch keep their existing JSON protocol. A generic SQL rewriter would create an unnecessary parser and would be unsafe for nested JSON or literals, so it is not used.

The normal import path already streams raw binary values and computes necessary data transformations locally. Whole-blob HASHBYTES restrictions are relevant to other activation/apply paths; changing those would widen this task without helping the traced import route. Own config apply and exact platform 8.3.27.1859 admission remain unimplemented by this proposal.

No additional concurrency guarantee is introduced. Preparation continues to require coordinated exclusive configuration work, and all existing candidate/source verification and final transaction guards remain in place.
