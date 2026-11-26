# The drop-in `config apply` after rcheck-6: typed errors, the backup options, 8.5

Debug build of the branch, the drop-in as `ibcmd-rs infobase config apply` (the program under the platform's
command line), lab databases `ibcmd_rs_04_rcheck_twin1` (БСП 8.3.27) and `ibcmd_rs_04_rcheck_b85` (БСП 8.5), dropped
afterwards.

| run | result |
|---|---|
| 8.3.27, a catalog change (`CodeLength` 9 -> 12) staged by the platform's `import files --partial`, `apply --force --dynamic=disable --recovery-backup=<file>` | `[INFO] Обновление конфигурации базы данных...`, `[ERROR] требуется штатный config apply: 5eab8a1b-...: [structure] Catalog._ДемоПартнеры: Properties/CodeLength: 9 -> 12 (a property no rule covers)`, exit 1; nothing written; the backup option is accepted and inert (the restructuring is refused first) |
| 8.5 clone, `--platform=8.5.1 --user="Администратор (обычное приложение)"`, nothing staged | `Обновление конфигурации базы данных не требуется`, exit 0 (before: the drop-in's own `Применение конфигурации базы платформы 8.5 не поддерживается`, exit 1) |
| 8.5 clone, a module comment staged by the drop-in's own `config import` (patch mode, 137.8 s, 9 622 rows), `apply --exclusivity=assumed` | `[ERROR] требуется штатный config apply: 66193438-....81401d17-...: [unknown] Configuration: row .81401d17-...: content changed (99664295 -> 99663207 bytes)`, exit 1, 113.7 s: the check's words for the known 8.5 stage difference (`restructuring-check.md`, finding 10), not a refusal of 8.5 |

Unit tests (`src/dropin/apply.rs`, `parse.rs`, `help.rs`; 39 in `cargo test --lib dropin`) and `tests/dropin_cli.rs`
(10) cover the sorting by type (`StructuralRefusal`, `NeedsNativeApply` for apply and repair, `BackupRequired`,
`ExclusiveAccessRefused` with and without sessions and inside a context, `ExclusiveAccessUnprovable`, an untyped error
that carries the old marker phrases is a failure), the parsing of `--recovery-backup` and `--i-have-a-backup` (file wins,
empty value malformed, only for `apply`), the options handed to the apply and the help.
