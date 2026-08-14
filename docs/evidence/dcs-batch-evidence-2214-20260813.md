# Batch clean-room DCS evidence — 8.3.27.2214 / XML 2.20 (2026-08-13)

Одна лабораторная сессия Parallels «Windows 11» собрала четыре новых DCS-корпуса.
Authority: 1С `8.3.27.2214`, `ibcmd.exe` SHA-256
`11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b`, locale `ru_RU`,
источник XML `2.20`. Все seed'ы — hypothesis-only; истина — байты платформы.

## Метод

Базовое XML-дерево получено загрузкой retained clean-room CF существующего корпуса в
отдельную ИБ и настоящим `ibcmd config export` (см. «Известное ограничение» ниже);
заменялся только `Template.xml`. Для каждой координаты — две независимые свежие файловые
ИБ: `infobase create --import` → `config save` → `config export`. Native `Template.xml`,
packed и unpacked raw body побайтово совпали между раундами у всех четырёх координат;
полные CF ожидаемо различаются межэкземплярными данными (тот же паттерн, что в
`dcs-area-template-appearance`). Retained артефакты и хеши — в `manifest.json`
каждого корпуса; см. «Приёмочная сессия компилятора» ниже для статуса
`compiler_acceptance`.

## Новые корпуса

| Корпус | База | Формат-факты платформы |
|---|---|---|
| `dcs-area-appearance-web-color` | dcs-area-template-appearance | Исходный префикс `web:` не сохраняется: платформа эмитит автогенерируемый `d8p1` с локальной декларацией `xmlns:d8p1` на элементе значения (`d8p1:Red`, `xsi:type="v8ui:Color"`). Item'ы appearance переупорядочены: цветовой параметр `ЦветТекста` выводится перед `Расшифровка`. |
| `dcs-area-multi-cell-appearance` | dcs-area-template-appearance | Дети `dcsat:tableCell` канонизируются в порядок Field → appearance независимо от порядка seed. Одинаковые appearance-блоки двух ячеек в native XML полностью дублируются. |
| `dcs-parameter-scalar-types` | dcs-core | Native `Template.xml` побайтово равен seed: канонические формы `xs:boolean`/`true`, `xs:decimal`/`100.5` c NumberQualifiers, `v8:StandardPeriod` с `v8:variant xsi:type="v8:StandardPeriodVariant">LastMonth` подтверждены без переформатирования. |
| `dcs-output-parameters` | dcs-core | `dcsset:outputParameters` сохраняет позицию после `dcsset:order` перед `StructureItemGroup`; платформа добавляет item'у явный `xsi:type="dcsset:SettingsParameterValue"`. |

## Приёмочная сессия компилятора

Отдельная VM-сессия (Parallels «Windows 11», 1С `8.3.27.2214`, locale `ru_RU`) прогнала
все четыре кандидата, скомпилированных `ibcmd-rs` (`compile_dcs`, вход — retained
round-2 native `Template.xml`, а не pre-platform seed: доказанный cohort-парсер каждой
координаты требует platform/storage порядок, а не порядок исходного seed). Каждый
кандидат наложен через общие overlay API в соответствующий retained clean-room CF,
загружен и применён в свежей файловой ИБ `ru_RU`, сохранён, затем `Report.DcsCorpus`
рекурсивно экспортирован тем же пиненным оригинальным `ibcmd`, что и в основной сессии.

Все четыре координаты приняты платформой с первой попытки (один платформенный проход,
без повторов); re-export побайтово совпал с `rounds.native_template_sha256` соответствующего
манифеста у всех четырёх. Блок `compiler_acceptance` (10 полей: `status`, `method`,
`candidate_body_unpacked_size`, `candidate_body_unpacked_sha256`, `candidate_cf_sha256`,
`platform_saved_cf_size`, `platform_saved_cf_sha256`, `reexported_template_size`,
`reexported_template_sha256`, `reexport_matches_two_round_native_template`) записан в
`manifest.json` каждого корпуса между `document_topology` и `cohort`:

| Корпус | `reexported_template_sha256` |
|---|---|
| `dcs-area-appearance-web-color` | `7ca981cac18c0df2715d355eb6cf97665f80bd5a027dc67764b322b818a51a25` |
| `dcs-area-multi-cell-appearance` | `a72d97cfe65a43326433ce1bcfe80f7adf6b6ecfa50074d5d092461db52080d0` |
| `dcs-parameter-scalar-types` | `7f4f83f5e8adcb21b0e9e848726c3da19d9cbb94996a08e69a84784fc4c9f1e0` |
| `dcs-output-parameters` | `bc27a20de1bb75a83b3727ac457db04791cdd092c64e2e31e5b58ebecf296ddb` |

Приёмка доказана только для этой ровно одной координаты каждого корпуса (см.
обновлённый `non_claims` в каждом манифесте) — не для прочих значений параметров,
позиций или множественных item'ов. Guest-каталог приёмочной сессии удалён, VM
остановлена; retained fixtures репозитория не изменялись сверх добавления блока
`compiler_acceptance`.

## Заблокированные координаты (evidence не получен, ничего не заявляется)

- **StyleItem reference**: в репозитории нет засвидетельствованной lexical-формы
  `style:<…>` для значений `v8ui:Color`; сессия по политике не выдумывала написание.
  Требуется отдельный дизайн (конфигурация с настоящим StyleItem либо внешний словарь).
- **Optional link properties**: единственное упоминание гипотетических полей
  `parameter`/`parameterListAllowed` — явный non-claim «UNPROVEN» в существующем
  манифесте. Требуется словарь EDT/Unica cross-evidence до любого VM-раунда.

## Известное ограничение, найденное сессией

`cf export` не эмитит `ChildObjects`/`InternalInfo` в `Configuration.xml`
(воспроизведено и на несвязанном корпусе `task-basic`), а `cf overlay`/`cf bootstrap`
не компилируют DCS `Template.xml` оффлайн. Поэтому базовые деревья для импорта
готовились настоящим `ibcmd config export`, без ручного изобретения структур.
Ограничение зафиксировано как отдельная задача; данный документ его не решает.

## Cleanup

Guest-каталог `C:\lab-batch` удалён (C: 119,5 → 123,6 ГБ свободно), VM остановлена,
retained fixtures репозитория этой (сбор evidence) сессией не изменялись. Отдельная
приёмочная сессия компилятора (см. выше) впоследствии добавила блок
`compiler_acceptance` в те же четыре `manifest.json` — единственное последующее
изменение retained fixtures этого батча.
