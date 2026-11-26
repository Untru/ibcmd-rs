# Actual filedb READ and native status evidence — 2026-10-22

## Setup

Base UI commit: `1456adc1`. Shared READ backend: original `644684f2`,
cherry-picked as `020b88fe`. Own worktree/target: issue-380. Native executable:
`/opt/1cv8/8.5.1.1592/ibcmd`. All DB/data/temp/profile paths were under the
external lab root; only a new disposable copy of committed `tiny.1CD.gz` was used.
No existing DB or private ERP fixture was modified or published.

The exact 8.5.1.1592 profile is unknown to the repository (explicit initialize
refusal); actual READ used supported release profile `8.5.1` (XML 2.21).

1. Decompress committed fixture into a new `$RUN/db/1Cv8.1CD`.
2. Native `infobase config export $RUN/baseline` with explicit
   `--db-path=$RUN/db --data=$RUN/native-data --temp=$RUN/native-temp`.
3. Native `infobase config export status --base=$RUN/baseline/ConfigDumpInfo.xml`:
   exit 0, empty stdout.
4. Copy baseline into `$RUN/native-edit` and `$RUN/source`. Append one comment to
   `CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl` in native-edit only.
5. Native `infobase config import $RUN/native-edit` with the same explicit paths:
   exit 0. This is a native whole-import mutation, not a Configurator GUI edit.
6. Append an XML comment to `$RUN/source/Configuration.xml` locally.
7. Query shared filedb RPC and refresh the actual VS Code extension host.

## Measured divergence (acceptance blocker)

Native status after import:

```text
modified: CommonModule.ФайловаяБазаПроверка
modified: CommonModule.ФайловаяБазаПроверка.Module
modified: Configuration.Configuration
modified: Language.English
```

| Object | RPC DB/baseline bytes | UI byte label |
| --- | --- | --- |
| CommonModule.ФайловаяБазаПроверка | modified | Изменён в базе |
| Configuration | same | Изменён в файлах (local XML comment) |
| Language.English | same | Совпадает |

The native full import changed metadata versions for additional objects while
these objects' exported bytes stayed equal to baseline. The current server API
cannot reproduce native version semantics. Native object/module/root names also
need an agreed mapping to tree objects. **This evidence does not complete #380.**
The UI explicitly identifies the current result as a byte preview. No native
executable or shell fallback is used in production.

Required next contract: expose native-compatible object version/ConfigDumpInfo
comparison from #362 through the server, then combine that DB-side evidence with
local/baseline file evidence. Parent coordination is required; #380 does not own
a parallel version parser or filedb adapter. Re-run true Configurator module/local
file acceptance after that contract is integrated.

## Validation

- Shared backend: 5/5 editor_file_db tests, 6/6 editor_server tests.
- JS: 13/13 including real CLI/RPC checks, formatting and diff checks.
- Extension host: actual rows-dir and synthetic six-state/filter/Explorer/
  cancellation/stale/multi-root cases. Opt-in native-mutated filedb test records
  UI labels and explicitly records `nativeVersionParity: false`.

Lab records are retained under the own run directory (`commands.json`, native
stdout/stderr, `rpc-evidence.json`, `ui-evidence.json`). No fixture DB is committed.

Operational note: the initial Rust check used the shared lab TMPDIR; the upstream
`tests/common::temp_dir` clears fixed fixture directories. Further checks used the
own `tmp/issue-380-run` directory. A help-discovery invocation with positional
`help` attempted the native default data lock and failed before import; all actual
native DB commands used the explicit paths above. No retry targeted defaults.
