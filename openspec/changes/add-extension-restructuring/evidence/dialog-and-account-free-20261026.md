# Dialog import and account-free SQL verification (2026-10-26)

The observed refusal combined a Version increment, a new common module and a
nested managed dialog on an existing own DataProcessor. Root property values
were otherwise unchanged. The saved source/native comparison identified only
Designer omissions (button Type, XML2.21 ShowCommandBar derived from None,
XML2.21 metadata interface mode Any) and lexical XML differences, including
an unused palette namespace declaration. Private source bytes are not fixtures.

## Native-generated regression

Pinned public corpus: Untru/1c-fixtures@3b9a77c87b7f8a7bd025a9dceb5114e998c64b2a,
PR Untru/1c-fixtures#2. Generated versions 1.2.0.4 -> 1.2.0.5 / 1.2.0.6 cover
both 8.5.1.1150 (XML2.21) and 8.3.27.2214 (XML2.20). The native two-round
proofs, source/CFE versions and combined 1.2.0.6 were independently verified.
Earlier inputs, CFE, native trees and SQL facts remain unchanged.

The root regression runs the production whole-tree compiler in four isolated
processes, verifies the entire proposal against its input, then compares every
exported file byte-for-byte to the canonical native target (ConfigDumpInfo is
derived and excluded). The narrowed .6 input omits only the proven defaults;
its XML2.21 metadata descriptor additionally removes the unused palette binding.
XML2.20 retains its own envelope and does not acquire XML2.21 properties.

Semantic comparison is restricted to recognized metadata/Form XML. Scalar text,
identities, child order, default namespaces and used prefix bindings remain
strict. Prefix use includes decoded QName-like text/attribute values and CDATA;
globally used prefixes retain every declaration, including shadowed scopes.
Unknown properties and opaque resources are not normalized away.

## SQL verification contract

Direct import/apply and direct LIVE continuation do not call authenticated
infobase info or forward application credentials. Process regressions reject
both operations and cover absent/legacy credential options. Exact agent build,
registration existence and live SQL schema are checked independently; summary
is not reported as a physical SQL binding. Evidence of the managed worker's verified physical registration remains
strong. Unbound RAS PIDs cannot exempt actual SQL sessions; direct LIVE requires
an empty actual SQL target, including idle handles. Saved SQL identity, chain,
schema, transaction and concurrency guards remain in place.

A read-only administrative summary of a protected registration also succeeded
without application credentials. No private application tests or SQL writes
were used for this follow-up. SQL preservation evidence remains the measured
structural update described in native-restructure-20261025.md; the follow-up
changes admission, serialization, reporting and preflight rather than its DDL.


Session absence is admitted only after proving server-wide DMV visibility:
SQL2012–2019 requires VIEW SERVER STATE, SQL2022 and later requires VIEW SERVER
PERFORMANCE STATE, or sysadmin. Unknown/unsupported version and unavailable
permission results refuse before session inspection, disruption or publication.
The utility does not grant permissions or request an infobase account. Source:
https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-sessions-transact-sql?view=sql-server-ver17#permissions

## Verification

- Four platform/case compiler processes: PASS; before-fix refusal and faithful
  output failures were captured before implementation.
- Namespace comparison: 3 focused tests PASS; used/default binding rejection
  cases remain strict.
- Measured native-plan report now includes Params, SchemaStorage and scheduled
  service writes. Its real-plan regression first failed on the missing table.
- Server-visibility regression first failed; 140 affected root tests then passed. A read-only SQL Server SELECT evaluated the exact production CASE on 405 literal version/role/permission combinations with 0 mismatches; independent review recomputed every expected row. No application tables, principals or permissions were changed.
- Full project gates: cargo xtask check: 7 PASS on c26a562d9beb582a116eafbc3560a77ee2503a3a; final cargo xtask ci: 28 PASS on f69ed90ea215d305a3b66fd5d140d35e7ff53f1e; root library 4,125 PASS, 0 failures, 16 existing ignored tests.
- Independent fresh-context review: APPROVE; independent final native-dialog/profile 20/20 and XML comparison 3/3 PASS, plus independent 405-row SQL matrix verification.

A full-suite run exposed shared offline test state in an SQL-client probe.
The probe now runs in its own process with every prior assertion retained.
Another test-only correction scopes palette omission to XML2.21, whose native
envelope declares it. Neither correction ignores a test or changes production
acceptance. CI logs and historical red/green evidence are retained locally;
hosted build/release checks remain visible in PR #584.
The subsequent integration with master #586 was tested at
5f22ff89a1e97f3c9cd5e056850f283d50823ebe: local CI 28 PASS, root library
4,124 passed / 0 failed / 16 ignored; the remaining root test moved into
ibcmd-1c-vocab and passes there. The dev debug=0 executable from that source
has SHA-256 3aa7a9e575131eeac252e725f51bdbdb7fc240b81ee1cd540b57d2de7efc32e5.
These are historical source-labelled results, separate from the earlier SQL
execution identities in native-restructure-20261025.md. Test/check gates run
their own compiled harnesses, rather than the SQL execution binary.
