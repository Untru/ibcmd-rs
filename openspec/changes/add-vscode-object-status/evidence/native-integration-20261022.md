# Native integration acceptance — 2026-10-22

## Dependencies and safety

UI base: a892dd98. #362 helper b46cd919 -> 9316ae33; #382 READ backend
644684f2 -> 020b88fe; settings test fix c085fccd -> c7f5b2e4; scoped LF fix
c094b326 -> 0e0a2ce9. Own worktree/build/TMPDIR/cache only. Native executable
8.5.1.1592; supported Rust release profile 8.5.1 (XML 2.21), not an invented exact
build profile. All native DB commands use explicit external DB/data/temp paths.
Only new copies of public committed tiny fixtures are mutated. No ERP data published.

## Version-only mismatch regression fixed

The historical native-read-20261022.md experiment changed Configuration/Language
versions without changing XML bytes. After helper integration, the real extension
host now reports module=database, Configuration=both (local XML comment),
Language=database. Typed native output matches these DB-side changes. The same
results pass with a pinned Git baseline. Before/after DB bytes are identical for
all UI reads. Native inventory is consumed in memory, no native process in UI.

## All six states on a second real filedb

Create a unique DB from tiny.1CD.gz. Add four common modules to a copied public
source and import/export with native 8.5.1.1592 to establish a complete baseline.
Unchanged native status is empty. Native selected import:

```text
infobase config import files --base-dir=$RUN/edit --partial
  CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl
  CommonModules/StatusBoth/Ext/Module.bsl
  --db-path=$RUN/db --data=$RUN/native-data --temp=$RUN/native-temp
```

Native status against baseline reports only:

```text
modified: CommonModule.StatusBoth
modified: CommonModule.StatusBoth.Module
modified: CommonModule.ФайловаяБазаПроверка
modified: CommonModule.ФайловаяБазаПроверка.Module
```

Independent source construction/edits and actual extension-host results:

| Object | Native/local evidence | UI state |
| --- | --- | --- |
| StatusClean | Unchanged DB version and local bytes | same |
| ФайловаяБазаПроверка | Native module edit; local baseline bytes retained | database |
| StatusFile | Native unchanged; local module comment | file |
| StatusBoth | Native edit plus independent local module comment | both |
| StatusOnlyDB | Present in DB; omitted from newly constructed source tree | databaseOnly |
| StatusOnlyFile | New local descriptor/body; absent from DB | fileOnly |

All six labels and Explorer decorations passed in the real VS Code extension
host. Changed-only excludes same. Native changed names agree for database/both;
clean/file names are absent from native output. UI refresh leaves 1CD bytes
identical. This is a real native selected-file mutation, **not Configurator GUI**.

Reproduction is supplied by `editors/vscode/test/prepare-native-six.py` and optional
`IBCMD_TEST_SIXSTATE_ROOT` host scenario. The script creates unique own artifacts,
never deletes files, and records commands/stdout/stderr/expected states. Run records
are retained in own issue380-six-* / issue380-native-* lab directories;
`ui-evidence.json` records allSixStates/nativeVersionParity true for this scope.

## Relevant checks

- 18/18 JS tests, including real CLI/RPC checks and native snapshot mismatch.
- 1 real tiny-filedb additive RPC test: byte compatibility, version-only differences,
  baseline binding, fullRequired, malformed reference, no DB mutation.
- 5/5 native helper, 5/5 shared filedb and 6/6 editor_server integration tests.
- 2 mapping/MSSQL guard unit tests. Scoped LF attributes verified for TS/JS/JSON.
- Extension host: real rows, two native filedb cases, Git baseline, all six states,
  filter/Explorer, fullRequired, cancellation/stale selection and multi-root removal.
- Independent reviews: DB version race, Git subtree, numeric token, reference binding,
  mixed MSSQL views and owner fallback concerns addressed.

## Remaining scope gates

MSSQL native explicitly refuses mixed active/main view before files/SQL. Owner
mapping accepts only exact canonical/row identities plus measured root/common-module
roles; unsupported or ambiguous coverage returns inconclusive, never parent fallback.
FullRequired is surfaced, not converted to clean. Wider row-role/source support,
Configurator GUI validation if required, parent review and PR authorization gate
remain explicit; no complete ERP browser/performance or release claim.
