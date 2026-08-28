# Остаток по диаграммам УТ после волны Bots/GraphicalSchemaField/Task

**Статус после волны 2026-08-25 (ночь): пункты 1-2 (GanttChart) ЗАКРЫТЫ И
ПОДТВЕРЖДЕНО ПОЛНЫМ `ut`-ГЕЙТОМ.** Оба нативных примера
(`АнализЖурналаРегистрации/ПродолжительностьРаботыРегламентныхЗаданий`,
`ДлительностьОтложенногоОбновления/ДиаграммаГанта`) декодируются и
переэкспортируются байт-в-байт. `ut`-прогон после всех фиксов этой волны
даёт **`exact=50458`** против базового `50456` (`differing` `442` →
`440`), разность exact-множеств против **неизменяемого**
`$D/baselines/d0457a6/ut.parity.json` показывает РОВНО ТРИ новых
exact-файла и СЛОМАНО=0:

```
+ DataProcessors/ПроверкаКонтрагента/Forms/Форма/Ext/Form.xml            (закрыт предыдущей волной, ещё не отражён в d0457a6)
+ Reports/АнализЖурналаРегистрации/Templates/ПродолжительностьРаботыРегламентныхЗаданий/Ext/Template.xml   (эта волна)
+ Reports/ДлительностьОтложенногоОбновления/Templates/ДиаграммаГанта/Ext/Template.xml                       (эта волна)
```

Реальный остаток УТ после этой волны -- **0 файлов** (плюс всё те же 439
host-dependent, которые НЕ дефекты). Это была ПОСЛЕДНЯЯ реальная
диаграмма/дефект в 1С:УТ 11.5.27.75 -- первая БОЛЬШАЯ конфигурация на 100%
побайтового паритета (за вычетом host-зависимости), 50 898 файлов.

**Урок волны, едва не стоивший ложного "готово"**: первый полный
`ut`-прогон ПОСЛЕ реализации 33-полевой обёртки (все юнит-тесты зелёные)
дал `exact=50456` -- БЕЗ ИЗМЕНЕНИЙ! Причина: `parse_moxel_drawing`'s
`fields[12]` (сырой слот "object", как он реально разбивается из ЦЕЛОГО
`<drawing>`-тюпла в настоящем документе) несёт ОДНУ ЛИШНЮЮ ОБЁРТЫВАЮЩУЮ
СКОБКУ вокруг `{19,...}` -- `{ {19,...} }`, один член без запятой верхнего
уровня -- В ОТЛИЧИЕ от обычного `Chart`, чей `fields[12]` -- `{{11},
{74,...}}` без такой обёртки. Фикстуры `raw/*-object-payload.txt` этой
папки -- это УЖЕ РАЗВЁРНУТЫЙ `{19,...}` текст: прошлая волна вырезала его
вручную поиском маркера `{19,` внутри ЦЕЛОГО документа (`cf extract
<cf> <uuid>.0` даёт целый MOXCEL-документ, не объект), а не через
`parse_moxel_drawing` -- поэтому юнит-тесты на этих фикстурах ни разу не
касались этой обёртки и были зелёными, пока настоящий `ut`-гейт молчал.
Найдено прогоном НАСТОЯЩЕГО `1cv8.cf` через полный
`parse_moxel_spreadsheet_text` (не изолированный payload) и наблюдением,
что `parse_moxel_drawing` возвращает `None` там, где изолированный тест
давал `Some`. Исправлено в `GanttChart`-ветке `parse_moxel_drawing`: снять
ещё один слой обёртки (`split_1c_braced_fields`, ровно один член) перед
вызовом `parse_moxel_gantt_chart`. Добавлены ДВА новых регрессионных теста
(`renders_gantt_chart_drawing_with_elements_{not_init,init}_from_real_wrapped_field`),
идущие через `parse_moxel_drawings`/`push_moxel_drawing_xml` -- НЕ через
изолированный payload -- именно чтобы эта регрессия не проскочила снова
незамеченной. Фикстуры для них -- РЕАЛЬНЫЕ, обёрнутые поля,
`raw/*-full-drawing-field.txt` + `native/*-drawing.xml` (целый
`<drawing>...</drawing>`, не только `<object>`), извлечены напрямую из
`/Users/untru/.1cv8/1C/1cv8/tmplts/1c/trade/11_5_27_75/1cv8.cf` через
`ibcmd-rs cf extract <uuid>.0`, не пересобраны вручную.

**Доктринальный урок**: изолированная фикстура (даже если она "сырые
байты платформы") доказывает, что читается ОДНА конструкция в ИЗОЛЯЦИИ, но
НЕ что закрылся ФАЙЛ -- это уже второй раз в этом проекте (см. урок про
форму `ПроверкаКонтрагента`, три диаграммных реквизита вместо одного, в
`docs/evidence/ut-diagram-remainder-20260825.md`'s более раннем разделе).
На этот раз проблема была не "не хватает конструкций", а "фикстура сама
устранила ту самую обёртку, которую нужно было разобрать". Единственный
надёжный гейт -- полный корпус через настоящий пайплайн, что и сделано
здесь ДО объявления пункта закрытым.

Оказалось, что закрытие GanttChart -- не только 33-полевая обёртка (пункты
1-2 ниже), но и РАСШИРЕНИЕ уже существующего `parse_moxel_chart`
(`{74,...}`-payload, используемый и обычным `Chart`, и вложенным в
`GanttChart` как `field[1]`): оба GanttChart-примера несут ПЯТЬ новых
реальных полей (`isShowTitle`, `ttlBorder`/`lgBorder`/`chBorder`,
`transparent`, `ttlFont`/`legFont`/`chFont`, `legendScrollEnable`/
`animation`, `elementsIsInit`) и один новый код `chartType` (`6`=
`Column3D`), которые предыдущие 13 корпусных записей никогда не
варьировали -- их раскладка в `tail[]` найдена сопоставлением с уже
существующей `validate_moxel_chart_v74_front`'s "expected"-таблицей (те
самые позиции, которые она НЕ проверяла, потому что 13-корпус никогда их
не менял). Особенно важная находка: `elementsIsInit` -- НЕ хардкод
(`true` всегда), а реальный флаг (`tail[89]=="0"`, независимый от
`isShowLegend`, который прошлая волна с ним спутала), и он же управляет
ПРИСУТСТВИЕМ (не только содержимым) `legendPlacement`/`titleAreaPlacement`
в XML, а также третьим независимым триггером для `valuesScale`
(`isShowTitle && elementsIsInit`, доказано всеми тремя наблюдаемыми
комбинациями этих двух флагов). Полная трассировка -- в доккомментариях
`MoxelChart`, `validate_moxel_chart_v74_front`,
`validate_moxel_chart_v74_post_prefix`,
`validate_moxel_chart_v74_rectangle_check` в `src/mssql_dump/moxel.rs`, и
в обновлённом `tests/fixtures/native-evidence/8.3.27.2214/
moxel-ganttchart-remainder/manifest.json`.

Коммиты: `fix(moxel-chart): decode the GanttChart-embedded plain Chart
payload`, `fix(moxel-gantt): decode the GanttChart wrapper` (см. `git log`
текущего worktree). Метод: python-реплика `split_1c_braced_fields` для
быстрой офлайн-токенизации сырых payload'ов без пересборки Rust на
каждой гипотезе (см. `moxel1c.py`/`wrapper_fields.py` в scratchpad этой
сессии, не в репозитории) плюс ОДИН `seed.sh`-контроль
(`gantt-control` -- `CommonTemplate` `GanttChartTest` на скелете
`Web_Service`, несущий побайтово `dlitelnost`-запись) для подтверждения
`verticalScrollEnable` независимым XML-переключением через платформу, а
не только сопоставлением двух нативных примеров.

**Не решено и оставлено неинтерпретированным** (не влияет на XML, оба
файла закрылись без этого): ~7 design-time cache-слотов в `tail[]`
(`84,86,87,88,90,92,93`, гейтятся `elementsIsInit`, но КОНКРЕТНЫЕ значения
не разобраны, когда флаг `true` -- см. `validate_moxel_chart_v74_front`);
аналогичный cache-слот `post[20]` и пятитокенное окно перед
`elementsChart`. Многие поля GanttChart-обёртки (`keepScaleVariant`,
`fixedVariantInterval`, `autoFullInterval`, `noneVariantChars`,
`noneVariantMeasure`, `verticalStretch`, `showValueText`, `extTitle`,
`showPointsText`, `showData`, `intervalTextRepresentation`) варьируются в
писателе как литералы, доказанные лишь ДВУМЯ примерами -- следующий
GanttChart-пример (если найдётся в sslbase/ssl/uh) может показать другие
коды. См. `non_claims` в манифесте фикстуры.

## Полный гейт -- ПОДТВЕРЖДЕНО

`ut`: `exact=50458` (было `50456`), `differing=440` (было `442`),
`missing=0`, `extra=0`, `50898` файлов с обеих сторон. Разность
exact-множеств против неизменяемого `$D/baselines/d0457a6/ut.parity.json`:
СЛОМАНО=0, НОВЫХ exact РОВНО ТРИ (см. список выше). `ws`/`mdm`/`wms`/
`sslbase`/`ssl` подтверждены БЕЗ РЕГРЕССИЙ этой волной (числа не
изменились против `$D/baselines/2ccd98f/*.parity.json`: `ws` 29/29, `mdm`
160/164, `wms` 226/226, `sslbase` 9573/38+6, `ssl` 12644/50+7). `bundled9`:
9/9. `cargo test --lib`: 2310 passed / 33 failed, тот же список имён, что
`$D/baselines/2ccd98f/fail-base.txt`. `cargo test -p ibcmd-schema -p
ibcmd-xml`: 108/108, 262/262. `cargo fmt --check`/`git diff --check`:
чистые. Инструментация (`eprintln!`/`PROBE`) снята, только
предсуществующие константы остались.

Коммиты этой волны (см. `git log` текущего worktree): `fix(moxel-chart):
decode the GanttChart-embedded plain Chart payload`, `fix(moxel-gantt):
decode the GanttChart wrapper`, `fix(moxel-gantt): unwrap the real
object-slot brace, close УТ 11.5.27.75`.

---

Статус после волны 2026-08-25 (вторая половина дня): паритет УТ поднят с 7
расходящихся + 1 невыданный до **3 расходящихся** (было 4). Bots, Tasks, оба
GraphicalSchemaField и пункт 3 ниже (`СравнительныйАнализМенеджеров`)
закрыты (см. коммиты `596bcc8`, `eaa6847`, `6778b19`, `7982d6c`, `292d807` и
фикстуры `tests/fixtures/native-evidence/8.3.27.2214/
{bot-predefined-picture,task-number-allowed-length-data-lock-mode,
graphical-schema-field-leftwidest-page,moxel-chart-series-count-zero}`).

**Статус после волны 2026-08-25 (вечер)**: пункт 4
(`ПроверкаКонтрагента/Forms/Форма`) ЗАКРЫТ ЦЕЛИКОМ И ПОДТВЕРЖДЕНО ГЕЙТОМ:
`ut`-прогон после всех фиксов этой волны даёт `exact=50456` против
базового `50455` (было `443` расходящихся, стало `442`), разность
exact-множеств против `$D/baselines/d0457a6/ut.parity.json` показывает
РОВНО ОДИН новый exact-файл --
`DataProcessors/ПроверкаКонтрагента/Forms/Форма/Ext/Form.xml` -- и
СЛОМАНО=0. Остаток УТ теперь **2 реальных файла** (оба GanttChart-макета,
пункты 1-2) + 439 host-dependent. GanttChart -- следующая цель.
Гейты (`ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut`/`uh`) прогнаны целиком после
каждого коммита, СЛОМАНО=0 на каждом -- сверено против **неизменяемого
снимка `$D/baselines/d0457a6/*.parity.json`**, не `$D/base789`
(координатор переприбивает `base789` после каждого слияния в основную
ветку; он уже уехал вперёд на несвязанных пакетах вроде потери английских
языковых элементов -- `uh` там 127753, а не 120592 как в зафиксированном
`d0457a6`-снимке). По пути обнаружился двенадцатый пункт (не
предполагавшийся заранее): та же форма несёт ЕЩЁ ДВА chart-атрибута
(`chartType=Gauge`, `realSeriesCount=0`, иначе уже полностью разобранной
формы) -- без них файл не закрывался целиком, несмотря на то, что целевой
`ДиаграммаПоказателей`-атрибут уже декодировался верно. Коммиты этой
волны: `fix(form-chart): decode realSeriesCount>0 on form Chart
attributes`, `fix(form-chart): decode chartType=Line and splineMode`,
`fix(form-chart): decode legendPlacement=Bottom, titleAreaPlacement, and
three show-mode fields`, `fix(form-chart): decode pointsScale`,
`fix(form-chart): decode valuesScale, seriesScale and Gauge chartType,
close pt.4` (см. `git log` текущего worktree).

## Пункт 3 закрыт (коммит `292d807`) -- читай перед пунктами 1-2 и 4

Зазор в 3 токена оказался НЕ одной причиной, а тремя независимыми, каждая
найдена отдельным семенем (по одной правке XML на семя, скелет
`Web_Service`, см. `docs/evidence/seed-configurations-method.md`):

1. **`has_extended_scales`** (`post[0]`/`post[1]`, `"14","2"` без
   `pointsScale`/`valuesScale`/`colorPaletteDescription`/явного
   `xLabelsOrientation`/`paletteKind` -- `"0","0"` с ними). Пара списков id
   шкал после `post[22]` имеет длину `N = 1 + series_count` (было зашито
   `N = 2`), сдвигая позицию осей и `elements*`-прямоугольников на
   `2*(2-N)` токенов.
2. **`is_title_init`** (`post[7..10)`, независимый от (1) флаг --
   `titleIsInit`/`legendIsInit`/`chartIsInit`, доказано семенем, которое
   меняет ТОЛЬКО эти три XML-элемента).
3. Плюс попутно найденные точечные баги в уже читаемых полях: новый код
   цвета `-23` = `style:ToolTipBackColor`; `isShowLegend`/`isAutoPointName`/
   `scaleColor`/`gaugeThickness`/`gaugeBushThickness` читались как
   константы, а не как поля; пятитокенная проверка перед `elementsChart`
   зависит от `isShowLegend`, а не от `series_count*point_count` (старая
   гипотеза `has_real_data` для этой проверки и для (1) оказалась неверна --
   опровергнута семенем `empty-no-extended-scales`, см. манифест фикстуры);
   автоматическое имя серии ("Pivot" вместо сохранённого) требует ОБА
   условия разом: `has_extended_scales` снят И есть хотя бы одна реальная
   серия.

Полная трассировка (какое семя что доказало, включая опровергнутые по пути
гипотезы) -- в `tests/fixtures/native-evidence/8.3.27.2214/
moxel-chart-series-count-zero/manifest.json` и в комментариях
`src/mssql_dump/moxel.rs` (`parse_moxel_chart`,
`validate_moxel_chart_v74_post_prefix`,
`push_moxel_chart_series_text_xml`). **Метод**: `cf extract <cf>
<uuid>.0` на семени даёт RAW MOXCEL-байты без платформы на машине;
семена сравнивались попарно (control vs один изменённый XML-элемент) --
это НАМНОГО надёжнее, чем сравнение с чужим (другого чарта) "рабочим"
примером, потому что изолирует ровно одну переменную. Все шесть семян
этой волны -- в `$D/seeds/chart-{control,1series,1point,legend-none,
init,minimal}` (сырые деревья -- `$D/seeds/chart-src/chart*`).

**Не решено и оставлено неинтерпретированным** (не влияет на XML, поэтому
не мешало закрытию пункта 3, но пригодится для GanttChart/пункта 4, если
там встретится): `post[41..43]` (три вложенных блока вида
`{2,0,0,2,{1,0},{1,4,0.5,0.5,...}}`) и `tail[84]`/`tail[86]`/`tail[87]`
(design-time координаты легенды/plot-area, невидимые в XML -- `legendPlacement`
пишется как голый enum без координат). См. manifest `non_claims`.

Метод подтверждён: `$S/kit/seed.sh` на скелете `Web_Service` с минимальными
синтетическими объектами (не копией реального дистрибутива -- копия часто не
проходит реимпорт из-за несвязанных полей, например `RowsPicture`/`Type` на
других контролах той же формы) даёт наблюдение с обеих сторон за минуты, а не
требует гадать. Смотри `docs/evidence/seed-configurations-method.md`.

Канонический стенд теперь `/Users/untru/Documents/ChatGPT/ibcmd-stand`
(`$D` ниже), а не `/private/tmp/...` -- чистильщик `/tmp` трижды стирал
scratchpad за эту волну. `$D/kit/seed.sh` уже поправлен на `$D`.

## Остаток: 2 файла (было 3 -- пункт 4 закрыт этой волной), оба GanttChart

```
Reports/АнализЖурналаРегистрации/Templates/.../Ext/Template.xml         -- GanttChart (макет)
Reports/ДлительностьОтложенногоОбновления/Templates/.../Ext/Template.xml -- GanttChart (макет)
```

`DataProcessors/ПроверкаКонтрагента/Forms/Форма/Ext/Form.xml` (пункт 4,
`ChartField`-форма) ЗАКРЫТ этой волной -- см. раздел "Пункт 4" ниже,
подтверждено полным `ut`-гейтом (`exact` 50455→50456).

Не путать с `DataProcessors/ПроверкаКонтрагента/Templates/ФинансовыйАнализ` и
`Reports/ДосьеКонтрагента/Templates/ФинансовыйАнализ` -- это host-зависимые
файлы (различаются только именем серии Pivot/Сводная), они уже точные и НЕ
входят в остаток; не путать их с одноимённым DataProcessor'ом выше, у
которого расходится **форма**, а не макет.

Пункт 3 (`СравнительныйАнализПоказателейРаботыМенеджеров/Templates/
СравнительныйАнализМенеджеров`, `Chart` с `realSeriesCount=0`) закрыт --
см. раздел выше.

## 1-2. GanttChart -- отдельный объектный код, декодера нет вовсе

`fields.get(11)` (uuid типа диаграммы) для этих двух записей --
`e5fdc112-5c84-4a16-9728-72b85692b6e2`, не `a8b97779-...` (обычный Chart) --
`parse_moxel_drawing`'s `match` на `"10"` не находит ветку и отказывает всей
отрисовке (`_ => return None`), роняя блок `<drawing>` целиком (299 и 292
строки соответственно).

**ИСПРАВЛЕНИЕ (волна 2026-08-25, вторая половина дня)**: сырые байты обеих
записей ПЕРЕСНЯТЫ и на этот раз СОХРАНЕНЫ фикстурой --
`tests/fixtures/native-evidence/8.3.27.2214/moxel-ganttchart-remainder/`
(`raw/*-object-payload.txt` + `native/*-object.xml`, `manifest.json` с
разбором). Прежняя формула обёртки была НЕПОЛНОЙ:

```
fields[12] = {19, field1..field32}   -- 33 ВЕРХНЕУРОВНЕВЫХ поля, не 3
field1 = {0, {11}, {74, ...обычный chart payload...}}   -- это ТОЛЬКО field1
```

То есть `object`-слот GanttChart -- код `19`, за которым следуют **32
дополнительных верхнеуровневых поля** (не одна тройка, как думала прошлая
волна); `field1` -- уже знакомая тройка `{0,{11},{74,...}}`, тот же
payload, что `parse_moxel_chart` умеет разбирать. `chartType` у одной
записи даёт `Column3D` -- новый код, не входящий в `moxel_chart_type`
(`0`/`9`/`38`); нужно вычислить сырое значение (`tail.get(2)`) и добавить
строку.

**Новое в этой волне** (прямое сопоставление с native XML, без семян):

* `field[2]` и `field[3]` -- "cache template" для `<d3p1:points>` и
  `<d3p1:series>` соответственно: вложенное число `baseData` совпадает
  побайтово (`4294927473`/`4294901761` в первой записи,
  `4294901761`/`4294901761` во второй) с `<d3p1:points><d3p1:value>
  <d3p1:baseData>` / `<d3p1:series><d3p1:value><d3p1:baseData>`. Остальные
  под-поля (`testMode`, `contentCacheItem`'s цвета, `autoText`,
  `useValuesReverseBehavior`) НЕ разобраны.
* `field[12]`, `field[13]`, `field[14]` -- ДАТЫ в чистом 14-значном формате
  `ГГГГММДДЧЧММСС` (без разделителей), совпадают побайтово (после удаления
  дефисов/двоеточий/`T`) с `<d3p1:fullIntervalBegin>`/`<d3p1:fullIntervalEnd>`/
  `<d3p1:visualBegin>` в ОБЕИХ записях -- формат даты найден и прост
  (просто вставить `-`/`-`/`T`/`:`/`:` на фиксированные позиции).
* `field[7]` -- похоже на `timeScale`/`level` (вложенный `{3,0,1,{8,...}}`,
  сидит прямо перед тремя датами; `field[8]`/`field[9]` выглядят как
  коды `measure`/`interval`) -- НЕ проверено вторым независимым способом.
* `field[4..12)` и `field[15..33)` -- НЕ тронуты вообще, только existence
  проверен.

Нативный XML доказывает: `<object xsi:type="d3p1:GanttChart">` = ровно
`<d3p1:chart>` (все поля обычного Chart, 1:1 с `<object xsi:type=
"d3p1:Chart">`), а ЗА ним ещё **~55 дополнительных элементов**, специфичных
для Ганта: `<d3p1:points>`, `<d3p1:series>`, `<d3p1:drawEmpty>`,
`<d3p1:timeScale>` (с вложенным `<d3p1:level>` --
measure/interval/show/line/scaleColor/...), `<d3p1:keepScaleVariant>`,
`<d3p1:fixedVariantMeasure/Interval>`, `<d3p1:autoFullInterval>`,
`<d3p1:fullIntervalBegin/End>`, `<d3p1:visualBegin>`,
`<d3p1:intervalDrawType>`, `<d3p1:backIntervals>`, `<d3p1:linksColor/Line>`,
`<d3p1:showPointsText>`, `<d3p1:showData>`, `<d3p1:textPlacement>`,
`<d3p1:intervalTextRepresentation>` и другие -- полный список теперь прямо
в фикстуре (`native/*-object.xml`), не только в `$D`.

**Объём**: это отдельный, большой декодер (33 верхнеуровневых поля обёртки
плюс ~55 XML-элементов с вложенными структурами и минимум один формат
даты, теперь известный), а не точечная правка. Всего 2 нативных примера в
УТ -- по доктрине этого мало для полной 33-членной записи; нужны семена с
вариациями (пустой Гант уже есть -- оба текущих примера пустые; нужен
минимум один НЕпустой семпл, если такой найдётся в sslbase/ssl/uh, и/или
семена с рукописным `Ext/Template.xml`, аналогично плану для пункта 3
выше).

## 4. Форма с диаграммой и сериями: `ПроверкаКонтрагента/Forms/Форма`

Отдельная конструкция от макетов -- это `ChartField` (форменное поле), не
`GraphicalSchemaField` и не MOXCEL-объект. Нативный XML:

```xml
<Settings xmlns:d4p1="http://v8.1c.ru/8.2/data/chart" xsi:type="d4p1:Chart">
    <d4p1:seriesCurId>7</d4p1:seriesCurId>
    ...316 строк, реальные 4 серии (НЕ пустой чарт)...
    <d4p1:valuesAxis/>
    <d4p1:pointsAxis/>
</Settings>
```

Блок `<Settings xsi:type="d4p1:Chart">` целиком (297 строк) отсутствует в
нашем выводе -- **ИСПРАВЛЕНИЕ (волна 2026-08-25, вторая половина дня)**:
это НЕ неисследованный путь. `src/mssql_dump/form_body.rs` уже несёт
`parse_form_chart_settings_xml`/`format_form_chart_settings_xml` -- рабочий
декодер ИМЕННО этого блока (`<Settings xsi:type="d4p1:Chart">` формного
атрибута/поля типа "Диаграмма"), с доказанной 197-членной раскладкой
(`FORM_CHART_TAIL_FIELDS = 197`, `FORM_CHART_TAIL_START = 18`) на ДВУХ
других чартах УТ с `realSeriesCount=0`
(`Catalogs/ВариантыАнализаЦелевыхПоказателей/Forms/НастройкаДемоДанных` и
`InformationRegisters/СезонныеКоэффициенты/Forms/СезонныеКоэффициенты`).
Payload -- ТОТ ЖЕ `{11},{74,...}`, подтверждено докстрингом функции: "The
payload is the same `{{11},{74,…}}` record the spreadsheet-document writer
already builds for a chart drawing". Раскладка 197-членного хвоста
(`realSeriesCount=0`) СОВПАДАЕТ по позициям с `parse_moxel_chart`'s
раскладкой для `series_count=0` из пункта 3 выше вплоть минимум до индекса
121 (`rebuildTime`) -- сверено вручную в этой волне, до применения к
`ПроверкаКонтрагента/Forms/Форма` дело не дошло.

Функция УЖЕ ЯВНО ОТКАЗЫВАЕТ на цели пункта 4: докстринг гласит "Scope: this
reader accepts exactly the shape both chart attributes of the 197-member
tail carry -- no series records, no point records -- and refuses anything
else, including the richer four-series shape
`DataProcessors/ПроверкаКонтрагента/Forms/Форма` stores, whose extra
elements (`realSeriesData`, `seriesScale`, `titleAreaPlacement`,
`valuesToolTipShowMode`, …) no second observation pins." То есть: слот
найден, формат частично разобран, но `realSeriesCount=4` (реальные, не
пустые серии) требует РАСШИРИТЬ этот СУЩЕСТВУЮЩИЙ декодер, а не писать
новый с нуля.

**ПРОВЕРЕНО этой волной**: `ДиаграммаПоказателей` -- ИМЕННО формный
АТРИБУТ (`<Attribute name="ДиаграммаПоказателей" id="34">` с `<v8:Type
xmlns:d5p1="...">d5p1:Chart</v8:Type>` и `<Settings xsi:type="d4p1:Chart">`
внутри него), как оба доказанных примера -- НЕ отдельный слот
`ChartField`-поля. Поле `ChartField name="ДиаграммаПоказателей"` в той же
форме -- просто UI-контрол с `<DataPath>ДиаграммаПоказателей</DataPath>`,
привязанный к этому атрибуту; его СОБСТВЕННЫЙ 11-членный кортеж (геометрия
extent/stretch) уже разобран `FORM_DOCUMENT_FIELD_GEOMETRY`/`"ChartField"`
и НЕ содержит `<Settings>` -- это отдельная, уже закрытая часть, к пункту 4
отношения не имеет.

**Метод пункта 3 переносится сюда напрямую**: семя на скелете `Web_Service`
с вручную заданными 4 сериями (скопировать `<d4p1:realSeriesData>` из
рабочего MOXCEL-примера, см. `docs/evidence/seed-configurations-method.md`
и фикстуру `moxel-chart-series-count-zero` этой волны как образец приёма
"одно изменение XML -- один семя -- один raw-diff").

### Пункт 4: большая часть закрыта семенами (волна 2026-08-25, вечер)

**ИСПРАВЛЕНИЕ найденной ранее ошибки индексации**: искомый чарт --
**ПЕРВОЕ**, не третье, вхождение маркера `{11},\r\n{74,` в тексте формы
(byte-offset ~674789 в UTF-8-декодированной строке; предыдущая запись
спутала `text.find` по кодовым точкам с байтовым смещением и взяла
третье вхождение, которое оказалось ДРУГИМ, пустым чартом на той же
форме -- отсюда ложное "`data[]` length=215" в старых заметках). `cf
extract` -- это СВОЙ (`ibcmd-rs cf extract`) подкоманда, а не `/opt/1cv8`
`ibcmd`; та печатает help на неизвестной команде и не извлекает ничего.
Верно: `holder = {"#", uuid, {11}, {74,...}}`, `data = holder[3]`, БЕЗ
второй обёртки как у MOXCEL. `realSeriesCount=4` (id 2,3,4,6), `data[]`
length=271, `cursor(tail_start)=62`, что совпадает с формулой
`18+11*series_count` (см. ниже) -- НЕ с зафиксированным ранее `62` по
отдельной причине, это ровно то же число, что и вывод из семян.

**Метод, который сработал**: семя `chart-form-control` (DataProcessor
`ChartFormTest`, Web_Service-скелет, ОДИН атрибут `Диаграмма` типа
`Chart`, `realSeriesCount=0`, скопирован байт-в-байт с уже доказанного
`НастройкаДемоДанных`) дал **100% с первого раза** -- существующий
декодер уже корректен для этого случая. Далее -- одно семя на один XML-
элемент поверх `chart-form-4series` (control с `realSeriesCount=4`,
id 2,3,4,6, ТОЧНО как у `ПроверкаКонтрагента`), т.е. натуральный target
воспроизведён как control, а не разбирался вслепую с 5+ переменными
разом. Все семена и raw/native пары -- в
`tests/fixtures/native-evidence/8.3.27.2214/{form-chart-series-count,
form-chart-linetype-splinemode,form-chart-placement-and-showmodes}/` с
регрессионными тестами `renders_form_chart_settings_with_*` в
`src/mssql_dump/tests.rs`.

**Закрыто этой волной (доказано семенами, 100% байт-в-байт round-trip,
уже в коде)**:

1. `realSeriesCount>0`: `N` реальных `realSeriesData` (11-членных, тот же
   формат, что и `push_moxel_chart_series_text_xml`) идут ПЕРЕД
   постоянным `realExSeriesData`-плейсхолдером, а не вместо него.
   `tail_start = 18 + 11*series_count`. Хвост после `rebuildTime` растёт
   на `3*series_count` членов -- ТА ЖЕ формула `moxel.rs`
   (`197 + 3*series_count + point_count*(1+4*series_count)`), теперь
   подтверждена семенами при `series_count=1` и `series_count=4` (не
   только сопоставлением с native XML без семян, как раньше). Рост
   раскладывается на (а) список из `1+series_count` `{0,<id>,0}`-записей
   плюс `1+series_count` `{0,0}`-записей сразу после `rebuildTime`
   (заменяет фиксированные 3 позиции `t[123..126)` базового случая) и
   (б) ОДНУ лишнюю копию иначе непричастного "funnel-link"-подобного
   30-членного блока позже в хвосте (было 1 копия, стало
   `1+series_count`). `tidx()` в `format_form_chart_settings_xml`
   транслирует старые фиксированные позиции в новые.
2. `marker` в `form_chart_series_xml` -- НЕ moxel-совместимый enum
   (`0..3`=None/Rect/Circle/Rhomb): семя `chart-form-1series` держит
   `marker=3` у реальной серии и `marker=1` у плейсхолдера ОДНОВРЕМЕННО,
   и ОБА платформа переэкспортирует как `<d4p1:marker>Auto</d4p1:marker>`.
   Поле теперь только валидируется как целое и всегда пишется `Auto` --
   та же трактовка, что уже была у цвета.
3. Гварды `t[82..85)`/`t[88..93)`'s 88/89/92 (литералы isTransposed/
   autoTransposition/legendScrollEnable и titleIsInit/legendIsInit/
   chartIsInit) СНЯТЫ: `chart-form-1series` доказывает, что `t[84]`,
   `t[88]`, `t[89]` при реальной серии несут те же непричастные
   design-time float-координаты легенды/plot-area, что и `tail[84]`/
   `tail[86]`/`tail[87]` у MOXCEL-чарта (см. пункт 1-2 ниже) -- НЕ
   хранилище этих флагов. Литералы остаются теми же на всех трёх
   `series_count` (0, 1, 4).
4. `chartType` код `"0"` = `Line` (семя `chart-form-linetype`, единственный
   изменённый токен против `chart-form-4series`).
5. `splineMode` -- фиксированный (не растущий с `series_count`) слот
   `t[110]`, ранее нигде не читаемый; пишется только когда `!= "0"`, код
   `"1"` = `SmoothCurve` (семя `chart-form-splinemode`, единственный
   изменённый токен).
6. `legendPlacement` -- третий код `"4"` = `Bottom` (семя
   `chart-form-legendbottom`, тот же `tidx(161)`, что и раньше).
7. `titleAreaPlacement` -- новый элемент, слот `tidx(162)` (сразу после
   `legendPlacement`), пишется только когда `!= "0"`, код `"8"` = `None`
   (семя `chart-form-titleareaplacement`).
8. `valuesToolTipShowMode`/`pointsDropLinesShowMode`/
   `valuesDropLinesShowMode` -- три независимых, каждый
   present-only-when-nonzero слота (индексы 1/3/4) внутри одного
   5-членного кортежа `tidx(183)` (индексы 0/2 непричастны, везде `0`).
   Коды: `valuesToolTipShowMode` `"2"`=`ShowOnHover` (семя
   `chart-form-vttsm`); drop-lines-пара `"1"`=`Show` (семена
   `chart-form-pdlsm`/`chart-form-vdlsm`) и `"2"`=`DontShow` (подтверждено
   НАТИВНОЙ записью `ПроверкаКонтрагента` -- у неё оба drop-lines
   явно `DontShow`, а не опущены, значит `DontShow` реальный код, а не
   "нулевое" отсутствие элемента).

9. `pointsScale` -- ЗАКРЫТ. Живёт в `t(tidx(139))`, первом из трёх
   otherwise-непричастных "funnel-link"-подобных 30-членных блоков
   (`post[41..43]` в старой moxel-нумерации). ТРИ семени поверх
   `chart-form-4series` control триангулировали раскладку: `chart-form-
   pointsscale` (`gridLinesShowMode=Show`+`gridLine(width=1,Dotted)`+
   `labelColor=#B4B4B4`, совпадает с native УТ), `chart-form-pointsscale-
   min` (`gridLinesShowMode=DontShow`+`gridLine(width=1,Solid)`
   default+`labelColor` опущен) и `chart-form-pointsscale-labelcolor`
   (то же, что `-min`, но `labelColor=#B4B4B4` добавлен -- изолирует
   `labelColor` от `gridLinesShowMode`/`gridLine`, поскольку ОДНОГО
   `-min`-семени было недостаточно отличить их слоты). Раскладка блока
   (22 подполя, когда `pointsScale` отсутствует из XML, → 23, когда
   присутствует): `[0..5)="2,0,0,2,{1,0}"` (обёртка, константа); `[5]`
   = `titleArea` (13-членный кортеж `{1,4,0.5,0.5,font,textColor,
   backColor,1,border,borderColor,4,2,0}`, разобран через
   `form_chart_scale_title_area_xml`, константа с default-значениями
   ДАЖЕ когда `pointsScale` отсутствует -- присутствие элемента решается
   не здесь); `[6]` = трёхзначное состояние (`"2"`=`pointsScale`
   отсутствует, `"1"`=`DontShow`, `"0"`=`Show`); `[7]`=`"0"` (константа,
   непричастно); `[8]`=`"1"` при присутствии (флаг "есть `gridLine`",
   всегда `1` в обоих `pointsScale`-семенах); `[9]`=`gridLine`
   (line-структура, ЦЕЛИКОМ отсутствует, не просто default, когда
   `[6]="2"` -- отсюда сдвиг подполей на 1; переиспользует
   `form_chart_line_xml`, расширенный вторым кодом стиля `"2"`=`Dotted`,
   единственное наблюдение); `[10]`/`[11]`=`{3,4,{0}}`/`{7,3,0,1,100}`
   (константы, непричастны -- возможно, второй, неиспользуемый пока
   font/color-слот); `[12]`=`labelColor` (через `form_chart_color`,
   `auto`-паттерн ОПУЩЕН из XML целиком, direct-RGB пишется --
   ЕДИНСТВЕННОЕ поле во всём декодере, где `auto` не пишется буквально);
   `[13]="2"` (константа). Все три семени + `chart-form-4series` дают
   100% байт-в-байт; полный гейт-свип (`ws`/`mdm`/`wms`/`sslbase`/`ssl`/
   `ut`) БЕЗ регрессий (новая безусловная проверка `titleArea` идёт по
   ВСЕМ существующим чартам корпуса, не только по новым семенам).
   Фикстура: `tests/fixtures/native-evidence/8.3.27.2214/
   form-chart-points-scale/`.

Пункты 1-9 подтверждены `cargo test --lib` (2264 passed / 33 failed,
тот же список, что и `$D/baselines/d0457a6/fail-base.txt`, после
`pointsScale`; см. пункты 10-11 ниже для `valuesScale`/`seriesScale`) и
полным прогоном гейтов (`ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut`/`uh`) с
проверкой разности exact-множеств против **`$D/baselines/d0457a6/
*.parity.json`** (НЕ `$D/base789` -- это подвижный указатель,
переприбивается координатором после каждого слияния и уже уехал вперёд
на несвязанных пакетах; `$D/baselines/d0457a6/` закрыт на запись и
зафиксирован на этой базе) -- СЛОМАНО=0 везде; остаток УТ на тот момент
СЧЁТНО тот же (443 = 439 host-dep + оставшиеся реальные, т.к.
`ПроверкаКонтрагента`/GanttChart ещё не закрыты целиком).

10. `valuesScale` -- ЗАКРЫТ. Живёт в `t(tidx(140))`, соседнем блоке сразу
    после `pointsScale`'s. Семя `chart-form-valuesscale` (`chart-form-
    4series` control + ТОЛЬКО `<d4p1:valuesScale>` с `showTitle=
    DontShow`+default `titleArea`+`labelFormat` заданным) меняет РОВНО
    ДВА подполя из 22 (длина блока НЕ растёт, в отличие от `pointsScale`
    -- у `valuesScale` нет условно вставляемой под-записи вроде
    `gridLine`): `[1]` (`"0"`=отсутствует, `"1"`=`showTitle=DontShow`;
    код `Show` не наблюдался) и `[13]` (`labelFormat`, тот же паттерн
    `form_chart_localized_xml`, что `lbFormat`/`lbpFormat`/`vsFormat`).
    Платформа зеркалит ТОТ ЖЕ текст в верхнеуровневый `vsFormat` (`t[39]`,
    уже читаемый) при импорте -- подтверждено native-переэкспортом, но
    `labelFormat` в коде читает СВОЙ слот, а не `t[39]`. Титул-область --
    тот же 13-членный кортеж и `form_chart_scale_title_area_xml`, что и у
    `pointsScale`. 100% с ПЕРВОГО семени.
11. `seriesScale` -- ЗАКРЫТ. Живёт в `t(tidx(141))`, следующем блоке.
    Семя `chart-form-seriesscale` (control + `<d4p1:seriesScale>` с
    default `titleArea`+`gridLine(width=1,Dotted)`+`showInChart=
    DontShow`) меняет `[8]` (флаг "есть `gridLine`", ТА ЖЕ форма, что
    `pointsScale`'s собственный `[8]`) с вставкой `gridLine`-записи
    (растит блок 22→23 подполей, ВТОРОЕ независимое подтверждение кода
    стиля линии `"2"`=`Dotted`) и последний слот блока (`"0"`=отсутствует
    → `"2"`=`showInChart=DontShow`; код `Show` не наблюдался). 100% с
    ПЕРВОГО семени.

Первая прямая проверка на РЕАЛЬНОЙ (не семенной) записи
`DataProcessors/ПроверкаКонтрагента/Forms/Форма`'s `ДиаграммаПоказателей`
(все пять переменных сразу: `realSeriesCount=4`, `chartType=Line`,
`elementsIsInit=false`, `splineMode`, `legendPlacement=Bottom`, ТРИ
scale-блока и show-mode-тройка одновременно -- та самая запись, которую
предыдущая волна намеренно не трогала вслепую) декодировала и
переэкспортировала байт-в-байт identично -- НО полный `ut`-гейт на этом
шаге ВСЁ ЕЩЁ показывал файл расходящимся (443, не 442)! Извлечена
НАПРЯМУЮ из `1cv8.cf` через `ibcmd-rs cf extract <cf> acd13c5d-
edf3-4c18-99d7-663ac866d5e8.0 <out>` (СВОЯ подкоманда, не `/opt/1cv8`
`ibcmd`) -- это ПЕРВОЕ, не третье вхождение маркера `{11},\r\n{74,` в
декодированном тексте формы (см. исправление выше). Фикстура: `tests/
fixtures/native-evidence/8.3.27.2214/form-chart-provkontr-target/`.

12. `chartType=Gauge` -- ЗАКРЫТ, найден `diff`-ом полного `ut`-вывода
    против native: ТА ЖЕ форма `ПроверкаКонтрагента/Forms/Форма` несёт
    ЕЩЁ ДВА (идентичных, кроме `rebuildTime`) chart-атрибута с
    `chartType=Gauge` -- код, не входивший ни в один из уже известных
    (`0`=Line, `6`=Column3D, `12`=Pie). Оба -- иначе полностью в уже
    разобранной `realSeriesCount=0` форме (215-членный `data[]`, БЕЗ
    scale-блоков и show-mode полей) -- единственным недостающим кодом был
    именно `chartType`. Семя `chart-form-gaugetype` (control + ТОЛЬКО
    `<d4p1:chartType>` на `Gauge`) даёт единственный изменённый токен:
    `"38"`=`Gauge`. Фикстуры: `tests/fixtures/native-evidence/
    8.3.27.2214/form-chart-linetype-splinemode/` (семя) и `tests/
    fixtures/native-evidence/8.3.27.2214/form-chart-provkontr-gauge/`
    (обе настоящие записи, напрямую).

**ПУНКТ 4 ЗАКРЫТ ЦЕЛИКОМ И ПОДТВЕРЖДЁН ГЕЙТОМ.** После добавления
`chartType=Gauge` полный `ut`-прогон (`zsh $D/kit/run.sh ut <worktree>
<выход>`) даёт `exact=50456` (было `50455`), разность exact-множеств
против `$D/baselines/d0457a6/ut.parity.json` -- РОВНО ОДИН новый
exact-файл, `DataProcessors/ПроверкаКонтрагента/Forms/Форма/Ext/Form.xml`,
СЛОМАНО=0.

**Вывод**: пункт 4 закрыт полностью, 12/12 отдельных находок подтвердились
семенами (11/12 с первого-второго семени; `pointsScale`'s `labelColor`
потребовала третьего для отделения от `gridLinesShowMode`/`gridLine`),
плюс итоговая проверка на настоящих записях (три chart-атрибута на одной
форме, не одна) и финальным гейтом. Метод семян по одному элементу поверх
`chart-form-4series` полностью себя оправдал -- но НАПОМИНАНИЕ для
следующей волны: проверка на изолированной фикстуре (пусть даже
единственной "настоящей" записи) НЕ заменяет полный гейт на файл целиком
-- в этом файле было ТРИ разных chart-атрибута, и фикс одного не закрыл
файл, пока не нашёлся второй/третий через `diff` полного вывода.
`chart-form-4series-src` (и его копии `chart-form-*-src`) остаются
готовыми скелетами для будущих находок такого рода.

## Порядок атаки (рекомендация)

1. ~~Пункт 4~~ ЗАКРЫТ И ПОДТВЕРЖДЁН -- см. раздел выше.
2. Пункты 1-2 (GanttChart) -- самый большой объём, делать последним; начать
   с воспроизведения обёртки `{19,{0,{11},{74,...}}}` и код `chartType`
   `Column3D`, затем идти по XML сверху вниз, member за member, с семенами
   на каждый спорный член (даты -- отдельная проверка формата). Тот же
   payload `{74,...}` внутри -- значит `parse_moxel_chart` (после волны
   2026-08-25) уже умеет `series_count=0`; GanttChart-обёртке нужен только
   свой код `19` и ~55 дополнительных членов поверх него.
