# Clean-room пробы StyleItem и optional link — 8.3.27.2214 / XML 2.20 (2026-08-14)

Третья лабораторная сессия Parallels «Windows 11» закрыла обе координаты, заблокированные
батчем 2026-08-13. Authority: 1С `8.3.27.2214`, `ibcmd.exe` SHA-256
`11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b`, locale `ru_RU`.
Протокол тот же: base tree настоящим `ibcmd config export` retained CF, две независимые
свежие ИБ на пробу, побайтовое сравнение native XML и raw body между раундами,
retained round-2 CF. Probe-написания были hypothesis-only и авторизованы дизайном по
cross-evidence (production-код legacy-парити для style-лексики; EDT metamodel
`DataCompositionSchemaDataSetLink` для имён link-полей — четыре доказанных имени
совпадают с EDT-именами побуквенно).

## Новые корпуса

| Корпус | База | Формат-факты платформы |
|---|---|---|
| `dcs-area-style-color-reference` | dcs-area-template-appearance | Стандартный style-элемент в Area appearance: `xsi:type="v8ui:Color"` со значением `style:NegativeTextColor` (авто-префикс в native). Storage side table хранит именованную лексему (`NegativeTextColor`), не UUID; storage-lexeme параметра `ЦветФона` — `BackColor` (четвёртый случай паттерна «кириллица в XML ↔ английский в storage»). |
| `dcs-area-style-item-uuid` | dcs-area-template-appearance | Custom StyleItem `CorpusAccent` (Kind Color, web:Red) в конфигурации + ссылка из appearance. Native XML неотличим по форме от стандартной ссылки (`style:CorpusAccent`), но storage хранит `0:<uuid>` — UUID-форма впервые подтверждена платформенными байтами. Retained дополнительно: native `StyleItems/CorpusAccent.xml` и `Configuration.xml`. |
| `dcs-link-parameter` | dcs-query-union-link | `<parameter>` и `<parameterListAllowed>` приняты; native re-export побайтово равен seed. EDT-квалификатор `unsettable` у `parameterListAllowed` платформой НЕ подтверждён: значение `true` сохранено. |
| `dcs-link-expressions` | dcs-query-union-link | `required=false` (non-default) сохраняется, не опускается. Платформа канонизирует порядок трёх новых детей в `linkConditionExpression, startExpression, required`. |

Все четыре пробы приняты с первой легитимной попытки; оба раунда каждой пробы
детерминированы на уровнях native XML, packed и unpacked body (S2 — также по
`StyleItems/CorpusAccent.xml` и `Configuration.xml`).

## Опровергнутая гипотеза (зафиксирована честно)

Предсказание порядка `ChildObjects` для конфигурации со StyleItem, выведенное из
возрастающего class_id `CONFIGURATION_SECTION_1` (src/compiler/root.rs), опровергнуто:
платформа выдала порядок `Language, StyleItem, Report`. Это формат-факт для будущей
работы над compiler root; никакая политика на основе опровергнутой гипотезы не создана.

## Отклонённая попытка (не засчитана)

Одна конструктивная ошибка исполнителя (пропущенная декларация `xmlns:style` на
value-элементе первой попытки S2) вызвала XDTO-ошибку платформы
(`Mapping lexical value 'style:CorpusAccent' to value type 'Color'`); попытка
отменена и не входит в evidence, ошибка сохранена дословно в артефактах сессии.

## Границы

`compiler_acceptance` у четырёх новых корпусов отсутствует до отдельной приёмочной
сессии. Прочие Kind StyleItem (Font/Border), несколько style-ссылок, списки значений
link-параметров, `linkItem` containment — вне доказанного объёма.

## Cleanup

Guest-каталог `C:\lab-probes` удалён (C: 118,7 → 122,5 ГБ), VM остановлена,
основной чекаут не изменялся лабораторной сессией.
