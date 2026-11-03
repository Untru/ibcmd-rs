# Контракт MCP-адаптеров

`src/mcp/platform.rs` отделяет portable admission policy от нативной проверки
handles. Матрица описывает разрешённые операции, но сама не является правом
доступа и не заменяет operator TOML или проверку текущей базы.

| Adapter | Target | Read | CONFIGSAVE stage |
| --- | --- | --- | --- |
| Unix descriptors | rows fixture | да, fixed binding | отказ |
| Unix descriptors | file database | да, fixed binding | только operator agent-write=true |
| Unsupported, включая Windows | любой | отказ | отказ |
| Любой | SQL | отказ | отказ |

Production выбирает adapter по host; параметры tool не могут менять adapter,
сервер, базу, settings, source root, auth, opt-in или confirmation. Pure lexical
path validation одинакова на всех ОС; она не доказывает отсутствие symlink,
reparse или замены предка. Это остаётся обязанностью нативного adapter.

`SnapshotAdapter`/`BoundSnapshot` задают требуемый интерфейс будущего adapter:
чтение через допущенные handles, opaque equality witness и повторная проверка
исходной binding/content, общий cumulative budget и cancellation. В этой ветке
нового Windows/SQL implementation нет. Synthetic contract tests не заменяют
настоящую платформенную приёмку, MCP-клиент или negative controls.

Работа над Windows file-db snapshot отдельно не включает MCP: требуются также
полные settings/source/target guards и review. Witness не является authorization
или стабильным cross-platform ID. Обещаний universal ABA, atomic filesystem CAS
или непрерывного Windows deny-move эта abstraction не добавляет.

Контракт различает `Target` и `TargetAndSource`: status/compare/import требуют
полный source census с финальной проверкой. Target-only adapter обязан отказать.
Metadata-only Windows ancestor handles не считаются доказанной deny-delete custody.
Зелёный Windows CI с cfg(unix)-исключёнными MCP tests не доказывает поддержку MCP.
