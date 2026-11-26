# MCP native write acceptance — 2026-10-22

Platform: `/opt/1cv8/8.5.1.1592/ibcmd`, macOS arm64, Rust1.95.
All build/cache/temp/data/database/output locations are under the external lab.
The test unpacks the repository-owned synthetic tiny filedb into a new unique
lab directory; no user database, private ERP, CF or private XML was used.

`real_mcp_client_stages_module_and_native_orchestrator_applies` starts the real
`ibcmd-rs mcp` subprocess through the official rmcp client/stdio transport. The
operator settings select one exact filedb/source root with agent-write=true.
The source module is edited; the client reads the old database bytes and imports
the selected object through the genuine MCP tool. The response reports staged,
not applied; the backup equals the pre-write database. The old session refuses
further calls, and an explicit new session reads the staged module bytes.

After closing the client, the external test orchestrator runs normal native
config apply and export with explicit --db-path, --data and --temp under the test
root. The edited module and all other source files except the separately audited
ConfigDumpInfo.xml compare byte-for-byte with native export. All four inventory
names/IDs remain present and unique; every version is40 hex characters. Exactly
the edited module version changes; all other inventory tuples retain versions.
No native process is launched by production Rust.

Final native evidence: `tmp/ibcmd-mcp-write-ae4797b4-8b79-4016-a4c4-a87589eb7767`
under `/Volumes/Work/Dev/ibcmd-rs-lab`. Native apply/export logs remain there;
`logs/issue-386-candidate-native-mcp.log` records6 passed real MCP tests.

Negative coverage includes missing/false opt-in; model settings/server/database/
source/confirm/agent-write overrides; policy revocation after session start;
consumed-descriptor mismatch; policy/target/source/root-alias changes between
prepare and publish; genuine SDK import cancellation before publication; stale
session after publication; and error-bit/publication reporting for recovery.
Shared tests cover after-final-check ancestor replacement, relative cleanup,
NOFOLLOW, backup-generation failures and cancellation. Independent combined
review found no remaining blocker within held-directory/advisory-lock guarantees.

Final gates:4065 root library tests pass (15 ignored);25 filedb unit+10 filedb
integration tests pass;25 default editor/staging/filedb regressions pass;
all-targets and filedb all-target Clippy with warnings denied pass. Physical
adapter policy guard+selftest and OpenSpec strict validation pass without a
baseline change. Logs: `issue-386-candidate-{root-lib,filedb,all-targets,clippy}.log`,
`issue-386-dirfd-regression.log`, `issue-386-physical-policy-current{,-selftest}.log`,
`issue-386-openspec-current.log` in the lab logs directory.

This does not claim atomic filesystem CAS against a noncooperating same-UID
process changing leaves inside the held directory. Protect directory permissions.
Panic can leave .ibcmd-rs.tmp/.ibcmd-rs.bak.new artifacts requiring inspection;
RAII unlocks descriptors but does not universally clean artifacts. Recovery path
strings can become stale after ancestor renames; inspect identities before retry.
Parent exact-head policy/confirmation review gates publication of enabled MCP
write support. No push, PR, issue closure, merge or release is implied here.
