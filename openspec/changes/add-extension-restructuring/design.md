# Design

The native platform is the oracle. Use synthetic versioned Coverage extension updates with multiple new catalogs, independent information registers and associated tableless metadata. Existing catalogs, registers and an own document contain control records before either updater runs. The update also adds a decimal(15,2) document attribute and an enum value; these accompanying changes must preserve every old field, record and enum identity. Source input versions must increase, and built CFE versions must match their sources.

The measured full update also includes a common module, HTTP service, subsystem and non-predefined scheduled job. Scheduled jobs own extension SQL tables and a global service mapping; they must participate in schema planning. Predefined jobs additionally require persistent instances and an activation-condition writer; admitting that unmodeled case would be incorrect, so it is diagnosed before staging. Base data-separation contributions must be measured explicitly because ordinary main common attributes and extension-enabled separators have different inheritance rules.

Separate pure layout/planning from SQL execution. Reuse the S1 DBNames allocator, schema readers, DDL renderer and transaction/CAS mechanisms when extension measurements demonstrate identical semantics. New self-contained logic belongs in a portable workspace crate; physical record slots are supplied by schema-owned facts. Capture extension DBNames, SchemaStorage and DBSchema, registry and service/cache effects before choosing adapters.

Apply must validate the entire proposed image and physical plan before publishing. DDL, metadata, registry, names, schema and cache changes are one atomic transaction, protected by exclusive ownership and optimistic guards. A failed command, trial or concurrent change leaves the old usable extension and its data intact. No schema gates are removed without an evidenced replacement.

The initial public regression represents the real update pattern without private application code. Verification includes multiple new objects in one update, preserved existing rows/references, writes and reads of new objects in a fresh platform session, native re-export and subsequent native apply. Exercise both measured builds independently. Unsupported destructive or unmeasured plans fail before mutation with concrete diagnostics; they do not silently drop data.

Rejected alternatives: delegating product commands to native ibcmd (breaks standalone architecture); staging new CFE without schema publication (unusable database); emptying existing data or reseeding after apply (cannot prove preservation); SQL-only table comparison without a platform runtime probe (misses registry/cache problems).


Structural admission retains the measured active/staged container pairing:
80327 on 8.3.27.2214, 80501 on 8.5.1.1150, whole-infobase extension scope.
Existing body-only legacy format support is unchanged. No implicit manifest
upgrade is performed; legacy structural writers need separate native evidence.

Whole-tree compilation also supports form-only additions on an existing own
DataProcessor. These objects do not own persistent SQL tables. The same native
pair changes Version and adds a common module; the complete compiled image,
including every root child and nested form asset, must re-export faithfully.
Root comparison uses XML semantics for syntax-only differences while retaining
leaf text, element ordering, identity, default namespaces, used prefix bindings and unknown properties.

Direct SQL writes use SQL Server authorization, without an application-account
login. Profile preflight reads the exact RAS build and selected cluster-level
infobase summary, then probes the separately specified SQL target. Summary data
proves registration existence, not the server/database mapping; reports expose
that distinction explicitly. Internally created managed-worker registrations
retain their stronger known physical mapping evidence. Direct live continuation
keeps the persisted SQL identity/backup-chain/schema guards while using the same
account-free registration check; an old checkpoint is not reported as a newly
verified physical registration mapping.
Prefixed namespace declarations unused throughout both compared documents are
lexical only. Prefix use includes element/attribute names and decoded QName-like
text/attribute values; default namespace declarations remain strict.

Session emptiness is evidence only after session visibility is established.
A shared fail-closed SQL predicate checks effective server permissions before
DMV-based preflight and inside rendered admission/disruption scripts. SQL Server
2019 and earlier require VIEW SERVER STATE; SQL Server2022 and later require
VIEW SERVER PERFORMANCE STATE. A known supported major version and sufficient
permission (or sysadmin) are required; NULL or unavailable proof is not success.
No SQL permissions are granted automatically. See the Microsoft
[dm_exec_sessions permission contract](https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-sessions-transact-sql?view=sql-server-ver17#permissions).
Final review admission: root ChildObjects membership changes must correspond
to actual admitted descriptor changes. Reject unpaired root-only edits and
duplicates, while retaining prior coordinated tableless removals/renames.
The complete compiled image must preserve requested values and ordering. A failed nonstructural proof requires a concrete replacement
schema plan; None does not authorize staging or publication. Structural SQL
uses SQL Server 2017 ordered STRING_AGG and modern catalog columns, so a
separate legacy-compatible capability read must establish engine major >=14
and database compatibility >=110 before submitting those queries. Code-only
updates retain their existing engine requirements; no server/database setting
is changed. Source: https://learn.microsoft.com/en-us/sql/t-sql/functions/string-agg-transact-sql?view=sql-server-2017
