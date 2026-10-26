# Support SQL Server 2012 direct import

## Why

Issue #572 records a TLS handshake failure and a SQL Server 2012 refusal of FOR JSON PATH before direct configuration import can start. Native XML/CF loading takes about an hour on the reported environment. The built-in import metadata reads should not require JSON features introduced after SQL Server 2012.

## What changes

- Add explicit `IBCMD_RS_MSSQL_COMPATIBILITY=2012`, validating actual SQL Server major 11 as the first query.
- Permit a different native-TDS encryption policy only with that selector; retain required encryption by default.
- Use typed rows for three full-folder import metadata queries only on a verified compatibility client; preserve existing JSON paths otherwise.
- Preserve verification, multipart binary handling and transactional ConfigSave publication.
- Keep independent apply/activation profiles and their modern SQL requirements outside this change.
