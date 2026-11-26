# ibcmd-rs Connections for VS Code

Minimal connection UI for [#378](https://github.com/Untru/ibcmd-rs/issues/378).
Requires VS Code 1.95+ and an `ibcmd-rs` executable speaking editor protocol **1.3**
(this checkout). A 1.0 server is rejected before a connection is attempted.

## Use

1. Compile with `npm ci && npm run compile` in this directory. Launch this directory
   as an extension development host (example below), or install a platform VSIX.
2. Open a **trusted, local workspace folder**. Set **ibcmd: Executable** in user or
   remote-machine settings to the executable path; this setting is machine scoped,
   so workspace content cannot select a program to execute. No shell is used.
3. Run **ibcmd-rs: Add Connection**. Supply a friendly name, SQL Server, database,
   platform and Windows or SQL authentication. SQL passwords go to VS Code
   `SecretStorage`; the prompt is masked. Cancel leaves the connection unchanged.
4. Use **Bind Workspace Connections** to select several existing databases for
   a folder, then **Select Workspace Connection**, or click the status bar, to
   switch. Multi-root workspaces first ask which folder to use. Fixed database
   names are supported; existing wildcard entries can be edited into fixed names.
5. **Reload Connections and Refresh Status** rereads the settings, reconnects the
   selected bound database and requests pending state. The displayed pending state
   is a snapshot, not a background monitor. **Forget Connection Password** deletes
   the selected login's saved password and closes the current session.

Windows integrated authentication uses the account running the server and needs
Windows. SQL authentication requires a user. This client always chooses auth
explicitly and never inherits a global SQL login/password for that connection.
An actual SQL query must succeed before the status shows a connected database.
No live SQL or Windows login was used to validate this initial client.

## Shared configuration and secrets

Database `server`, `name`, `platform` (and existing `rows-dir`) remain in the normal
`ibcmd-rs.toml` format. Add/Edit writes the **workspace folder's** file; an inherited
entry is copied into that file. `settings show` and the editor read the usual
executable/AppData/workspace settings layers. The editor ignores `IBCMD_RS_CONFIG`
and removes `IBCMD_DB_PSW` from the server environment, so it uses workspace
settings and SecretStorage. To compare CLI output, unset an explicit
`IBCMD_RS_CONFIG` override or point it at the same workspace file.

Friendly names, auth mode, SQL user and bindings are editor preferences in
`.vscode/settings.json`, scoped to the folder. They do not require new TOML keys
and remain compatible with older CLI versions:

```json
{
  "ibcmd.connections": ["sql01/demo", "sql01/test"],
  "ibcmd.connectionMetadata": {
    "sql01/demo": { "label": "Development", "auth": "sql", "user": "developer" },
    "sql01/test": { "label": "Test", "auth": "windows" }
  }
}
```

Secret keys include the folder URI, database ID and SQL user; credentials are
not silently reused between projects, users or servers. Editing a login replaces
its password; changing the identity removes the previous secret after saving.
No password is placed in arguments, TOML, workspace settings, error messages or
extension output. Server stderr, `log` notifications and server error text are
not forwarded. The only password-bearing transport is the child process's stdin.
VS Code manages encryption/backing storage for SecretStorage.

Edit preserves comments and other keys by replacing only AST value spans.
Settings are saved with a flushed sibling temporary file and atomic rename.
A sibling exclusive lock serializes this client's writes, and exact snapshot
checks reject concurrent disk changes; dirty TOML editor buffers are rejected.
Inline `database = [{...}]` syntax can be read but Add/Edit refuses it rather than
rewriting it or dropping comments: use `[[database]]` tables. Existing duplicate
IDs must be resolved manually. If a process is forcibly killed during a save,
inspect and move its stale `.ibcmd-lock`/`.ibcmd-*.tmp` artifacts aside before retrying.
An unrelated writer that ignores the lock can still race in the tiny interval
between the final comparison and rename: portable filesystems provide no
compare-and-swap replacement. Avoid simultaneous external writes to this file.
TOML and VS Code settings/SecretStorage are separate saves, not a cross-file
transaction; after an interrupted save, inspect the TOML and rerun Edit/Bind.

Settings changes close the current server. Re-select or Reload to initialize
fresh settings. Switching/reloading disposes the old process because the protocol
has no disconnect method. Late answers cannot replace the current selection.
Pending errors, inconclusive verdicts and offline rows sources show **pending
unknown**, never “no pending changes”. A crashed server shows **disconnected**.

## Development and checks

All caches and host data can be placed beneath one chosen directory. On the
ibcmd-rs lab Mac, source `lab-env.sh`, use `CARGO_TARGET_DIR=$LAB/build/issue-378`,
and use an `IBCMD_TEST_ROOT` inside `$LAB/tmp`. No global npm extension installation
or VS Code profile change is needed.

```sh
# From the repository root; build the binary and generate offline rows from
# committed CF fixtures (no SQL Server).
cargo test --locked --test editor_server --no-default-features
export IBCMD_TEST_BINARY="$CARGO_TARGET_DIR/debug/ibcmd-rs"
export IBCMD_TEST_ROWS="${TMPDIR:-/tmp}/ibcmd-external-editor-server-protocol/rows/demo"
export IBCMD_TEST_ROOT="${TMPDIR:-/tmp}/ibcmd-vscode-tests"
cd editors/vscode
npm ci
npm test
npm run format:check
# Use installed VS Code, or omit this to download a test host into IBCMD_TEST_ROOT.
export VSCODE_EXECUTABLE_PATH="/Applications/Visual Studio Code.app/Contents/MacOS/Code"
npm run test:host
```

`npm test` covers TOML preservation/conflicts, UTF-8 framing, cancellation,
timeouts, crash, error redaction, generations and pending verdicts. With
`IBCMD_TEST_BINARY`, it also checks actual `settings show` and `serve --stdio`.
The extension-host suite drives Add/Edit/Bind/Select/Forget flows with controlled
UI responses, actual SecretStorage and CLI readback, plus a real offline
connection. Synthetic extension-host fixtures currently require POSIX; Windows
is covered by CLI/RPC and unit CI, not by a local Windows host run. Its **explicitly synthetic** server covers pending/error/crash and
switch races; these tests are not evidence of a SQL login. The host gets isolated
`user-data-dir`, `extensions-dir`, crash and temporary directories. Test evidence
is retained in the selected temporary root for inspection.

For an isolated manual development host after compilation:

```sh
code --extensionDevelopmentPath="$PWD" \
  --user-data-dir="$IBCMD_TEST_ROOT/manual-user-data" \
  --extensions-dir="$IBCMD_TEST_ROOT/manual-extensions" /path/to/workspace
```

Import, apply, selective export and packaged installation are described below.
Live SQL verification remains an independent acceptance gate.

## Object status (#380)

The Explorer **ibcmd-rs Objects** view is a minimal flat list of full names. Select
an existing connection, configure the source and reference for that workspace
folder, then run **Refresh Object Status**. **Reload Connections and Refresh
Status** also refreshes objects after reconnecting. The toolbar toggles **Changed
Objects Only**; unknown remains visible. Matching source files receive Explorer
badges. The separate configuration browser is described below; representative
ERP performance remains an acceptance gate.

```json
{
  "ibcmd.sourceDirectory": "src",
  "ibcmd.statusBaselineDirectory": "/external/reference-export"
}
```

The reference is a separate export from the last synchronization, including its
`ConfigDumpInfo.xml`. Refresh never updates it. Alternatively use
`ibcmd.statusGitRef` (commit/branch/tag) and an absolute `ibcmd.statusCacheDirectory`
under the external lab root. Git takes precedence over the reference directory;
its ref is pinned to one tree before reading committed source blobs. There is no
checkout, hook, shell or native process in production. Inspectable snapshots are
retained for user-controlled Trash cleanup.

| Label | Badge | Evidence |
| --- | --- | --- |
| Совпадает | = | Native DB versions and local files both equal reference |
| Изменён в базе | Б | Native DB version changed; local files equal reference |
| Изменён в файлах | Ф | Native DB version unchanged; local files changed |
| Изменён в базе и файлах | БФ | Native DB version and local files both changed |
| Только в базе | +Б | DB object has no local files |
| Только в файлах | +Ф | Local object is absent in DB |
| Статус неизвестен | ? | Missing/inconclusive/unsupported/stale evidence |

Native mode is the default. It reuses #362's typed ConfigDumpInfo comparison.
Native version changes are independent of file bytes: importing a configuration
can restamp Language/Configuration without changing their XML. Local/reference
bytes are compared separately, including additions, deletions and binary files.
The server brackets its reads with native snapshots; both client RPC comparisons
must agree on the object token, complete native snapshot and reference identity.
These hashes/tokens check freshness; they are never used as native configVersion.

Cancellation, RPC failures, malformed replies, changed references/snapshots,
symlink paths, missing folders and unsaved source editors clear old badges and
cannot become clean. Connection/configuration/workspace changes invalidate old
refreshes. A native `fullRequired` response stays explicit and common-object
change direction becomes unknown rather than unchanged.

Status reads use `ibcmd.statusTimeoutSeconds` (default 600 seconds, integer range
30–3600) for each source/reference request. Invalid runtime values use the default.
The progress notification remains cancellable; cancellation clears badges and
late responses cannot republish them. Other RPC methods retain their existing
30-second default. Windows dirty-editor and decoration paths are URI-normalized
and compared without casing, with directory/drive boundaries preserved; POSIX
path casing stays significant.

### Supported scope and remaining gates

The shared filedb READ backend is integrated from #382 `644684f2`; native helper
from #362 `b46cd919`. No duplicate backend/parser or production native process is
provided. Canonical owner helper #362 `a5fdd226` supplies immediate/native/root
owners, including ordinary object modules and form bodies. Supported nested files
roll up explicitly to the intended UI objects. Unmapped coverage is inconclusive.

Native SQL status uses the same main configuration view for version, tree and byte
reads, through the existing read-only CONFIGSAVE overlay and session-owned main cache.
Main and active exports remain separate and each checks its versions digest before
reuse; native before/after snapshots remain required freshness checks. Legacy
byte/tree requests retain their active view. SQL adapter tests use a recording
client that rejects writes; a live SQL server was not tested. For explicit byte-only
inspection, set `ibcmd.statusComparison` to `bytes`; that mode remains a labelled
preview. Native mode never silently falls back to byte equality.

Actual 8.5.1.1592 filedb acceptance covers all six UI/Explorer states, native
selected module imports plus independent local edits, folder/Git baseline,
changed-only filtering, and byte-identical DB after READ. It uses supported release
profile `8.5.1`; exact build 8.5.1.1592 is not yet in the profile registry.
This is native command validation, not a Configurator GUI run. Parent review,
live SQL and any required GUI acceptance remain explicit evidence gaps.
Evidence: `openspec/changes/add-vscode-object-status/evidence/native-integration-20261022.md`.

### Reproduce native acceptance (opt-in)

Keep all paths in the external lab. Existing Rust fixture tests use fixed scratch
names, so use a dedicated `TMPDIR` for #380. No global extension install is needed.
After building/testing the binary and generating rows as described above:

```sh
export IBCMD_TEST_NATIVE_BINARY=/opt/1cv8/8.5.1.1592/ibcmd
export IBCMD_TEST_ROOT="$LAB/tmp/issue-380-native-acceptance"
python3 editors/vscode/test/prepare-native-six.py
export IBCMD_TEST_SIXSTATE_ROOT="$(cat "$IBCMD_TEST_ROOT/native-six-run-path")"
export IBCMD_TEST_BINARY="$LAB/build/issue-380/debug/ibcmd-rs"
export IBCMD_TEST_ROWS="$TMPDIR/ibcmd-external-editor-server-protocol/rows/demo"
export VSCODE_EXECUTABLE_PATH="/Applications/Visual Studio Code.app/Contents/MacOS/Code"
npm run test:host --prefix editors/vscode
```

The generator creates a unique disposable DB from committed public tiny fixtures.
Every native command has explicit DB/data/temp paths. It preserves commands,
stdout/stderr, expected states and source/reference trees; it deletes nothing.
The host records `ui-evidence.json` and checks that UI reads do not mutate 1CD bytes.
`IBCMD_TEST_FILEDB_ROOT` optionally adds the separate version-only whole-import
regression case documented in the historical `native-read-20261022.md` evidence.

Opt-in Catalog/module/form owner regression (new disposable DB only):

```sh
IBCMD_TEST_ROOT=/absolute/external/owners \
IBCMD_TEST_NATIVE_BINARY=/opt/1cv8/8.5.1.1592/ibcmd \
IBCMD_TEST_BINARY=/absolute/external/build/debug/ibcmd-rs \
python3 editors/vscode/test/prepare-native-owners.py
```

This public fixture test checks identical and platform-modified inventories, an
unrelated common module and DB byte immutability. Artifacts are retained.

### Deferred Git baseline performance work

Each refresh currently creates a retained snapshot and reads blobs in individual
Git processes. Cross-refresh reuse and batched extraction are deferred to #379:
a safe reusable cache needs completed-only publication, tree+repository+prefix
identity, and an integrity/partial-cancellation policy. The current pinned-tree,
path/link guards and Trash-controlled retention remain; this PR does not claim
ERP-scale Git baseline performance.

## Открытие исходников из базы

В меню объекта или файла дерева выберите «Открыть из базы». Редактор откроет
виртуальный адрес `ibcmd-rs` без выгрузки на диск. Виртуальные файлы доступны
только для чтения; после смены подключения или обновления дерева откройте
их заново. Для подсветки BSL используйте установленное языковое расширение.

«Сравнить с базой» у файла открывает стандартное окно сравнения. У объекта
или группы сначала появляется список изменённых и отсутствующих файлов.
Отсутствующая сторона показана пустым документом. При сравнении группы
сначала сохраните местные правки; сравнение отдельного файла использует
документ редактора, включая несохранённый текст. Двоичные файлы читаются
без преобразования, их отображение зависит от средств VS Code.

`npm run test:source-host` проверяет команды в VS Code с настоящим сервером.
Предварительно запустите `cargo test --locked --no-default-features --test
editor_server`: он создаст необходимые сохранённые строки и эталон в
системном временном каталоге. Укажите `IBCMD_TEST_BINARY` и `IBCMD_TEST_ROOT`.

## Дерево конфигурации

Представление «Конфигурация 1С» находится в проводнике. После подключения оно
показывает базу; команда `ibcmd.tree.xml` открывает каталог штатной XML-выгрузки.
Группы читаются при раскрытии. В разделе «Общие» собраны общие объекты,
остальные виды показаны отдельными группами. Реквизиты и табличные части
доступны только для просмотра; модули, формы и макеты видны у владельца.

Команда поиска читает имена и синонимы, в том числе ещё не раскрытых объектов.
Поиск может требовать чтения большого числа описаний. Повторное раскрытие
использует кэш; кнопка обновления перечитывает дерево. Смена подключения
отменяет прежние запросы. Для изменений в базе требуется явное обновление;
изменения XML/BSL внутри открытой рабочей области обновляют местное дерево.

Переход к местному файлу использует `ibcmd.sourceDirectory` относительно
папки рабочей области; по умолчанию это сама папка. Для XML-источника переход
идёт в выбранный каталог. Отсутствующий файл не создаётся автоматически.
Сервер для дерева должен включать добавленное поле `source/read.files`.

Проверки: `npm test`, `npm run test:tree-host`. Последняя команда требует
`IBCMD_TEST_ROOT` для изолированных настроек; `VSCODE_EXECUTABLE_PATH` позволяет
указать установленный VS Code. Без него проверка скачивает отдельный экземпляр.
Состав дерева проверен на небольшом сохранённом штатном примере. Замер на
ERP УХ до двух секунд и сверка её полного состава остаются условиями приёмки
задачи № 379; соответствующая база здесь отсутствует.


## Selective export (#381)

Set `ibcmd.exportCacheDirectory` to an absolute staging folder outside
`ibcmd.sourceDirectory` (use the external laboratory disk). Select object rows
with Ctrl/Cmd or Shift, then **Выгрузить из базы** in the context menu.
**Выгрузить группу из базы** opens a group picker; **Выгрузить изменённые из базы**
uses the source's `ConfigDumpInfo.xml` as its native version reference. Local byte
changes alone do not select a database object for changed export. A database
removal/rename uses the native full-conversion fallback.

The new `objects/exportPlan` RPC writes only an empty, separate staging folder;
its response lists `objects`, `files`, `removed`, and original SHA-256 fingerprints
(`expected`, null for missing targets). The client checks affected editor buffers
and Git staged/modified/untracked/ignored paths before publication. Dirty buffers
must be saved or reverted first; Git changes require an explicit modal decision
listing every affected path. Existing files outside Git are treated as unversioned
conflicts. Git execution failures fail closed. Disk changes, connection changes
and cancellation invalidate the plan. Equal bytes are left untouched.

Partial export updates selected version entries and keeps unsynchronized entries,
including nested parts, at their previous versions. It does not advertise the
whole database as synchronized. Inventory is published last. Deletions use VS Code
Trash; original bytes are retained under the staging directory's `originals/`.
Interrupted publication can leave some files updated; use those retained originals
to recover, then repeat export. This is a trusted local workspace operation, not a
transaction against hostile concurrent filesystem replacement. Linked target
components are refused. No source tree scan is required for one-object export.

`IBCMD_TEST_SUITE=export-host-suite.js npm run test:host` runs actual VS Code dirty
buffer protection and real RPC group export on the repository's own disposable
tiny database. The standard host runner gives it a fresh profile/workspace below
`IBCMD_TEST_ROOT`; all fixtures and staging evidence are retained for Trash cleanup.
ERP UH document latency and SQL acceptance require separate representative lab
fixtures; the tiny public test does not establish those results.
### Selected and changed import (#382)

For a Unix file database, add a fixed `[[database]]` entry with `db-path` in
workspace TOML and select that connection. `ibcmd.sourceDirectory` defaults to
`.` and may name a configuration source folder inside the workspace. Explorer
and editor context menus provide «Загрузить в базу»; «Загрузить изменённые» is
also available. The #380 object-view hook accepts its `{object, uri}` rows.

Import prepares and verifies a complete effective CONFIG/CONFIGSAVE baseline,
then asks for confirmation before publishing. Dependency suggestions require
separate consent and another check. Unsaved files, switched connections,
changed source/target and unsupported changes fail before publication. New
objects/deletions and unsupported file collations are refused.

Changed import requires synchronized `ConfigDumpInfo.xml` versions; it refuses
stale database/staging versions rather than reverting them. Refresh the export
baseline after synchronization, or explicitly select files when reviewing a
conflict. Successful import leaves CONFIGSAVE staged; apply is available through the separate
**ibcmd-rs: Apply CONFIGSAVE** command (#384). File-db pending state is refreshed
after import and apply.

For local evidence collection, set `IBCMD_TEST_KEEP_FIXTURES=1` before the Rust
file-db, partial-import and source-traversal tests. Their newly owned fixture
directories are then retained, including after assertion failure. Without this
flag, disposable CI fixtures are cleaned on scope exit. This test-only switch
does not change production temporary-file cleanup or recovery artifacts.

## Platform VSIX packaging and installation

Tooling requires Node.js 22+ and Python 3. The engineering package uses locked npm dependencies, an esbuild CommonJS bundle
with only `vscode` external, and `vsce --no-dependencies`. Python 3 normalizes ZIP
entry order, timestamps, permissions and compression. Given the same source,
locked toolchain, binary and `SOURCE_DATE_EPOCH`, repeated VSIX files have identical
SHA-256. This does not claim that binaries built by different Rust toolchains are
identical. Staging directories and test profiles are retained for inspection.

```sh
npm ci
npm run bundle
npm run test:package
# Build the release binary on Windows using Rust 1.95.0:
# cargo build --locked --release --no-default-features --target x86_64-pc-windows-msvc
npm run package -- --target win32-x64 \
  --binary ../../target/x86_64-pc-windows-msvc/release/ibcmd-rs.exe \
  --out /path/to/artifacts/ibcmd-rs-win32-x64.vsix
```

Windows packages contain `bin/ibcmd-rs.exe`; macOS arm64 lab packages contain
`bin/ibcmd-rs`. A binary for the wrong target is rejected. Choose **Extensions →
Install from VSIX** in VS Code or Cursor, then open a trusted local workspace,
bind/select the laboratory connection and refresh **ibcmd-rs Objects**. Packages
are platform specific; a Windows VSIX is not a Mac installation candidate.

Set **ibcmd-rs: Path** (`ibcmd-rs.path`) in user/machine settings to use another
executable. Empty uses the bundle, then `ibcmd-rs` on PATH for development builds.
The existing machine setting `ibcmd.executable` remains supported, below
`ibcmd-rs.path` in precedence. Neither setting accepts a shell command.

[Offline tree illustration](https://github.com/Untru/ibcmd-rs/blob/master/editors/vscode/images/offline-tree.svg) (based on asserted fixture object names).

The question marks above are honest unknown states without a comparison baseline;
seeing the tree does not establish native version equality or a live SQL login.
This is an illustration, not a screenshot. Computer Use screenshot access was
unavailable in the lab execution environment.

`test:installed` installs the actual VSIX into a fresh extensions/profile directory,
loads only a separate empty test driver as a development extension, and asserts
that the production extension was loaded from the installed location. It verifies
the bundled executable, fixture inventory/tree and explicit path override.

```sh
export IBCMD_TEST_ROOT=/external/lab/tmp/installed-check
export IBCMD_TEST_VSIX=/external/lab/artifacts/ibcmd-rs-darwin-arm64.vsix
export IBCMD_TEST_ROWS=/external/lab/tmp/ibcmd-external-editor-server-protocol/rows/demo
export VSCODE_EXECUTABLE_PATH="/Applications/Visual Studio Code.app/Contents/MacOS/Code"
# Cursor: /Applications/Cursor.app/Contents/MacOS/Cursor
npm run test:installed
```

CI builds and compares two Windows VSIX files, checks installation/tree in a clean
Windows VS Code host and uploads VSIX/checksum plus acceptance JSON. The existing
release workflow packages the Windows release binary into a VSIX and includes it
in the audited-artifact download/attachment path. No Marketplace/Open VSX publishing
or publisher credentials are configured here. The owner must create accounts and
perform publication; a release must still pass the repository release gates.

## Source-specific editing and configuration views

Edit Connection supports SQL and read-only rows-directory entries. Omitted servers
continue to inherit normal settings, and copied inherited rows-directory entries
retain their resolved directory. File databases and unknown source types are
excluded from this editor. The TOML writer also refuses `db-path` replacements,
including when a local entry changes source while a prompt is open.

Without a native comparison baseline, object status and tree requests both use the
active configuration. With a native baseline, both use main configuration and the
server's version-evidence path. Missing version evidence remains unknown.


## Apply CONFIGSAVE (#384)

After importing, the status bar refreshes the CONFIGSAVE pending indication.
Use **ibcmd-rs: Apply CONFIGSAVE** from the command palette. The confirmation
names the infobase, backend and exclusive mode. The `ibcmd-rs Apply` Output
channel reports start, completion and uncertain outcomes without forwarding
configuration data, native output, server error chains or credentials.

- SQL connections call `config/apply` using the existing own #343 backend.
  Exclusive session checks and the existing structural gate remain enforced;
  this editor route does not enable restructuring, assumed exclusivity,
  session termination or dynamic mode. Recovery artifacts are retained in a
  fresh `.ibcmd-recovery/<uuid>` directory inside the connected workspace.
- File databases use native `ibcmd` **from the extension**, after consent.
  Set machine-scoped `ibcmd.nativeExecutables`, mapping the bound platform
  version (for example `8.5.1`) to an absolute executable path from that
  platform installation. Verify that mapping before use. The server only
  returns the connected file database's descriptor and never launches native
  tools. Native `--data` and `--temp` are fresh subdirectories under the
  workspace's `.ibcmd-recovery`, retained for inspection.
- Stored-row folders cannot apply. The command exposes exclusive mode only;
  dynamic compatibility is tracked separately in #347.

Keep a usable backup and close file-database sessions before consent. Cancel
before publication stops the request at a checkpoint; native cancellation
terminates the process. Once SQL execution or native publication has begun,
cancellation/timeout/disconnection can leave an uncertain outcome. Refresh
pending state and inspect recovery before retrying. A completed apply is
never converted into cancellation merely because a late cancel arrived.

Live SQL acceptance requires an authorized disposable SQL endpoint and is
not established by a file-database/native acceptance run. No ERP content is
needed or distributed by these tests. `.ibcmd-recovery` may contain private
recovery material: retain it locally and keep it outside published sources.


Opt-in native acceptance (fresh public `tiny-source`, no teardown): set
`IBCMD_TEST_BINARY` to the candidate, `IBCMD_TEST_NATIVE` to native ibcmd
8.5.1, and an absolute external `IBCMD_TEST_ROOT`, then run
`npm run test:native-apply`. It creates an `en_US_POSIX` disposable database,
imports an edited module through the real client/RPC, verifies that declining
apply changes no database bytes, applies through the client native route,
and compares the complete native export with the edited source. The
version manifest is checked separately for preserved names/IDs and the
changed module version. The script forces fixture retention for the server.

Local retained-fixture validation uses the focused `editor_server` integration
target, not the destructive root-unit test
`file_import::tests::owned_scratch_is_removed_on_success_error_and_unwind`.
That test's default cleanup assertions remain intact and apply only when
`IBCMD_TEST_KEEP_FIXTURES` is unset. The default production cleanup behavior is
unchanged; the explicit lab retention switch keeps import scratch evidence.
