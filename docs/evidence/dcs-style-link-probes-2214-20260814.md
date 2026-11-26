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

## Приёмочная сессия компилятора

Четвёртая лабораторная сессия (Parallels «Windows 11», 1С `8.3.27.2214`, locale `ru_RU`)
прогнала `compile_dcs`-кандидатов всех четырёх координат на retained round-2 native
`Template.xml` через общие overlay API в свежую файловую ИБ `ru_RU`, один платформенный
проход на координату, re-export тем же пиненным `ibcmd`, что и в основной сессии.

| Корпус | Статус | `reexported_template_sha256` / ошибка |
|---|---|---|
| `dcs-area-style-color-reference` | ACCEPTED | `4269ac193b76bb88ecaaf65a5b4ef9ed12a31cdcf1d36d8ac429de68cf10f970` (= `rounds.native_template_sha256`) |
| `dcs-link-parameter` | ACCEPTED | `381e86721884c63c9f99dcde21f1cd78cca07b4644714bf635e954b1f59fc698` (= `rounds.native_template_sha256`) |
| `dcs-link-expressions` | ACCEPTED | `e80cc9492ab93cabff9799fb14e7e4c6fafff0d96129acba19ba53d4aa4faf54` (= `rounds.native_template_sha256`) |
| `dcs-area-style-item-uuid` | ACCEPTED | `98f1857d3424198275cc35834a6635c28623568aae8d01a95cb5e220f91b818f` (= `rounds.native_template_sha256`) |

Все четыре координаты приняты платформой; re-export побайтово совпал с
`rounds.native_template_sha256` соответствующего манифеста у всех четырёх. Блок
`compiler_acceptance` (10 полей: `status`, `method`, `candidate_body_unpacked_size`,
`candidate_body_unpacked_sha256`, `candidate_cf_sha256`, `platform_saved_cf_size`,
`platform_saved_cf_sha256`, `reexported_template_size`, `reexported_template_sha256`,
`reexport_matches_two_round_native_template`) записан в `manifest.json` каждой из
четырёх координат между `document_topology` и `cohort`.

`dcs-area-style-item-uuid` в четвёртой сессии вернулась `NOT-COMPILABLE`: `compile_dcs`
отклонял retained round-2 native `Template.xml` до попытки построения кандидата CF,
VM-загрузки, сохранения или re-export, потому что тогдашний адаптерский путь не нёс
resolver для custom-StyleItem/uuid-координаты — та же асимметрия, что уже
задокументирована для decode-направления в `def4e7b`. Одиннадцатая сессия закрыла этот
разрыв: `2fc495a` (минимизация primary/settings документов в storage-форму) сделала
компилированный кандидат побайтово равным genuine plaintext, а отдельно найденный и
устранённый баг гарнесса — `cf overlay --raw-asset`, получавший уже упакованный
(дважды-deflate) байт-поток вместо PLAINTEXT-входа, из-за чего `pack_raw_deflated_blob_from_bytes`
сжимал его повторно — объяснял наблюдавшуюся в 7-й и 10-й сессиях `Stream format error`
при `config export`. После подачи в `--raw-asset` именно PLAINTEXT (однослойная
компрессия) полный цикл compile→overlay→import→save→export прошёл чисто с первой
попытки: `Template.xml`, `Configuration.xml` и `StyleItems/CorpusAccent.xml` совпали с
retained-хэшами манифеста побайтово. Манифест `dcs-area-style-item-uuid` теперь несёт
блок `compiler_acceptance` наравне с тремя остальными координатами; устаревшая
NOT-COMPILABLE-формулировка снята из его `non_claims`.

Приёмка доказана для всех четырёх координат этого корпуса — не для прочих
значений, позиций или для более широкого style-reference-resolver'а, который пока не
обобщён за пределы этой одной evidenced координаты.

## Границы

`compiler_acceptance` есть у всех четырёх новых корпусов (см. «Приёмочная сессия
компилятора» выше). Прочие Kind StyleItem (Font/Border), несколько style-ссылок, списки
значений link-параметров, `linkItem` containment — вне доказанного объёма.

## Cleanup

Guest-каталог `C:\lab-probes` удалён (C: 118,7 → 122,5 ГБ), VM остановлена,
основной чекаут не изменялся лабораторной сессией.
