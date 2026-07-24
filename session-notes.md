# Session Notes - 2026-07-09 19:16:04 +03:00

## Current Task
Bring `ibcmd-rs` form XML export closer to native `ibcmd` for BSP forms, especially:
`AccountingRegisters/_ДемоЖурналПроводокБухгалтерскогоУчетаБезКорреспонденции/Forms/ФормаСписка/Ext/Form.xml`.

## Completed
- Found that native `ibcmd` does not emit empty owned-form metadata `<ExtendedPresentation/>`; changed form metadata formatting so empty `ExtendedPresentation` is omitted for ordinary owned forms while parsed non-empty values are preserved.
- Added generic parsing of button `RepresentationInContextMenu` from the extended button layout slot: `0=None`, `1=AdditionalInContextMenu`, `2=OnlyInContextMenu`.
- Changed `DynamicList` settings formatting so explicit `ManualQuery=false` is preserved and empty `<QueryText></QueryText>` is not emitted.
- Fixed `UsualGroup` output order so `VerticalStretch` is placed after `Title`/`HorizontalStretch`, matching the native XML ordering observed in the BSP form.
- Narrowed wrapper `55` table `CommandBarLocation` output to the layout shape where native `ibcmd` actually emits it.
- Narrowed wrapper `55` table `FileDragMode` suppression: omit the default `AsFile` only when the split-head layout marker is absent, root defaults match, and slot `53` says there is no explicit file-drag mode.
- Kept constructor updates in existing tests only where required by changed structs; no new test work should be added now.

## Pending
- Do not write tests and do not run tests until the user explicitly asks. User wants the main export logic finished first.
- Continue reviewing remaining full-export diffs against native `ibcmd`, prioritizing model/raw-field rules over object-name special cases.
- If more form mismatches appear, inspect raw inflated form blobs and bind XML output to slots/common model shape before patching.

## Next Action
Continue from `src/mssql_dump/form_body.rs`: inspect the next native-vs-generated form diff, identify the raw-field pattern, then patch the general parser/formatter logic without adding or running tests.

## Key Decisions
- Treat these as schema/model parity issues, not per-object exceptions.
- Do not hardcode form names such as `ФормаСписка`; use wrapper/layout markers and raw slots shared by all forms.
- Current user instruction from 2026-07-09: do not write tests and do not run tests now; tests will be handled later.

## Modified Files
- `src/mssql_dump/form_body.rs`
- `src/mssql_dump/mod.rs`
- `src/mssql_dump/tests.rs`
- `session-notes.md`

## Retrospective — 2026-07-24

- Симптомный цикл «XML diff → соседний raw slot → локальный патч» оказался неверным: рост exact-файлов не доказывал правильность декодера.
- Конкретная корневая ошибка: `Table.AutoMaxWidth` читался из slot 53, хотя строгая схема уже определяет его как `EnableDrag`; slot 54 — размер property bag.
- Полный production-backed provenance corpus окупился: 5 020 форм и 4 372 Table теперь проверяются повторным запросом за секунды, а не ручными выборками.
- Обязательный gate для нового правила: ноль смешанных native policies на полном УТ и сохранённом БСП; до этого production export не менять.
- Stale binary и debug corpus без промежуточного checkpoint дважды потратили время; дальше запускать release binary и проверять embedded/source commit до долгого прогона.
- Результаты субагентов нельзя принимать по заявлению: нужны независимое ревью, baseline-сравнение падений и воспроизводимые команды.
- README-процент отражает только полный parity run, а не покрытие тестами, корреляцию corpus или оценку закрываемого кластера.
- Универсальное правило для проекта: schema-owned field names и bounds должны быть единственным источником slot semantics; числовые индексы в production parser запрещены без accessor и fixture evidence.
