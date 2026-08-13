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
каждого корпуса; `compiler_acceptance` намеренно отсутствует до этапа реализации.

## Новые корпуса

| Корпус | База | Формат-факты платформы |
|---|---|---|
| `dcs-area-appearance-web-color` | dcs-area-template-appearance | Исходный префикс `web:` не сохраняется: платформа эмитит автогенерируемый `d8p1` с локальной декларацией `xmlns:d8p1` на элементе значения (`d8p1:Red`, `xsi:type="v8ui:Color"`). Item'ы appearance переупорядочены: цветовой параметр `ЦветТекста` выводится перед `Расшифровка`. |
| `dcs-area-multi-cell-appearance` | dcs-area-template-appearance | Дети `dcsat:tableCell` канонизируются в порядок Field → appearance независимо от порядка seed. Одинаковые appearance-блоки двух ячеек в native XML полностью дублируются. |
| `dcs-parameter-scalar-types` | dcs-core | Native `Template.xml` побайтово равен seed: канонические формы `xs:boolean`/`true`, `xs:decimal`/`100.5` c NumberQualifiers, `v8:StandardPeriod` с `v8:variant xsi:type="v8:StandardPeriodVariant">LastMonth` подтверждены без переформатирования. |
| `dcs-output-parameters` | dcs-core | `dcsset:outputParameters` сохраняет позицию после `dcsset:order` перед `StructureItemGroup`; платформа добавляет item'у явный `xsi:type="dcsset:SettingsParameterValue"`. |

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
retained fixtures репозитория не изменялись.
