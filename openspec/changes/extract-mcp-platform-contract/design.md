# MCP platform contract

The production selector chooses the existing Unix descriptor adapter or an
unsupported adapter. Target kinds are rows fixtures, file databases and SQL.
Read and CONFIGSAVE staging are separate capabilities: Unix rows are read-only,
Unix file databases require fixed binding and explicit operator opt-in to stage,
and SQL/unsupported adapters admit neither. The matrix is a policy predicate,
not an I/O capability token. Synthetic adapter values cannot select a production
adapter through tool parameters.

Settings, policy, actual consumed target identity, source root and final census
remain checked by the existing adapter. Generic equality checks receive only
operator-bound/native evidence; model-supplied identifiers are not evidence.
Snapshot identity is an opaque equality witness, not authorization or a stable
cross-platform identifier. Snapshot adapters must capture through admitted held
handles, revalidate the same binding and content, share cumulative budgets and
check cancellation between chunks and immediately before publication. The
required Rust trait is an interface contract with no permissive default or
Windows implementation; its synthetic test implementation performs no real I/O.

Portable lexical path checks reject absolute paths, drive/UNC/mixed separators,
empty/dot/parent components and NUL. They do not prove filesystem confinement:
symlink/reparse, alias and ancestor checks still belong to the real adapter.
Only each tool's schema key is admitted. Confirmation, auth, server, target,
settings and opt-in overrides remain rejected; confirmation comes from operator
TOML, never from a model-supplied boolean.

Checked budget accounting commits no charge after overflow/limit refusal.
Cancellation is checked before accounting/snapshot work. Existing cancellation,
publication/recovery semantics and limits remain authoritative. A published
recovery failure cannot be relabelled as safe cancellation.

Windows file-db snapshot work is separate. Its proposed local NTFS HANDLE/hash
witness does not prove MCP source/settings confinement or full-chain mutation
safety. Windows and SQL remain unsupported here until their own real-platform
negative controls, acceptance and independent review pass. No unconditional ABA,
atomic filesystem CAS, directory durability or continuous move-denial claim is
made by this abstraction. No native platform process runs in production Rust.

The snapshot API distinguishes Target from TargetAndSource. status/compare/import
require the latter: target read admission alone is insufficient without a bounded
source census and final revalidation. A target-only synthetic adapter refuses
that scope. Existing real MCP source census and post-census policy checks remain.
Windows metadata-only ancestor handles are not treated as evidence of deny-delete
custody; that adapter's correction and full-chain review remain outside this slice.
Windows functional MCP tests are currently cfg(unix)-excluded; green Windows CI
alone does not prove MCP support. These portable tests prove policy predicates,
not native handles, ACLs, same-byte replacements, reparse/case/trailing-dot alias
confinement, SDK disconnect publication semantics, or recovery behavior. Those
remain explicit real-adapter review/acceptance gates.
