# Остаток по диаграммам УТ после волны Bots/GraphicalSchemaField/Task

Статус после волны 2026-08-25 (вторая половина дня): паритет УТ поднят с 7
расходящихся + 1 невыданный до **3 расходящихся** (было 4). Bots, Tasks, оба
GraphicalSchemaField и пункт 3 ниже (`СравнительныйАнализМенеджеров`)
закрыты (см. коммиты `596bcc8`, `eaa6847`, `6778b19`, `7982d6c`, `292d807` и
фикстуры `tests/fixtures/native-evidence/8.3.27.2214/
{bot-predefined-picture,task-number-allowed-length-data-lock-mode,
graphical-schema-field-leftwidest-page,moxel-chart-series-count-zero}`).

**Статус после волны 2026-08-25 (вечер)**: остаток всё ещё **3 файла**
(закрытие любого из трёх требует ЦЕЛОГО байт-в-байт совпадения файла, а
не частичного прогресса), но пункт 4 (`ПроверкаКонтрагента/Forms/Форма`)
продвинут с "не начат вслепую" до "9 из ~10 отдельных находок закрыты
семенами (включая `pointsScale` целиком), осталось `valuesScale`/
`seriesScale`" -- см. раздел "Пункт 4" ниже. GanttChart (пункты 1-2) не
тронут. Гейты (`ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut`/`uh`) прогнаны
целиком после каждого коммита, СЛОМАНО=0 на каждом -- сверено против
**неизменяемого снимка `$D/baselines/d0457a6/*.parity.json`**, не
`$D/base789` (координатор переприбивает `base789` после каждого слияния
в основную ветку; он уже уехал вперёд на несвязанных пакетах вроде
потери английских языковых элементов -- `uh` там 127753, а не 120592
как в зафиксированном `d0457a6`-снимке). Коммиты этой волны: `fix(form-
chart): decode realSeriesCount>0 on form Chart attributes`, `fix(form-
chart): decode chartType=Line and splineMode`, `fix(form-chart): decode
legendPlacement=Bottom, titleAreaPlacement, and three show-mode fields`,
`fix(form-chart): decode pointsScale` (см. `git log` текущего
worktree).

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

## Остаток: 3 файла, все диаграммные

```
DataProcessors/ПроверкаКонтрагента/Forms/Форма/Ext/Form.xml            -- ChartField (форма)
Reports/АнализЖурналаРегистрации/Templates/.../Ext/Template.xml         -- GanttChart (макет)
Reports/ДлительностьОтложенногоОбновления/Templates/.../Ext/Template.xml -- GanttChart (макет)
```

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

Все девять пунктов подтверждены `cargo test --lib` (2263 passed / 33
failed, тот же список, что и `$D/baselines/d0457a6/fail-base.txt`) и
полным прогоном гейтов (`ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut`/`uh`) с
проверкой разности exact-множеств против **`$D/baselines/d0457a6/
*.parity.json`** (НЕ `$D/base789` -- это подвижный указатель,
переприбивается координатором после каждого слияния и уже уехал вперёд
на несвязанных пакетах; `$D/baselines/d0457a6/` закрыт на запись и
зафиксирован на этой базе) -- СЛОМАНО=0 везде; остаток УТ пока СЧЁТНО
тот же (443 = 439 host-dep + оставшиеся реальные, т.к.
`ПроверкаКонтрагента`/GanttChart ещё не закрыты целиком).

**НЕ закрыто (пункт 4 всё ещё открыт, но сузился до одной пары структур)**:

* `valuesScale`/`seriesScale` -- по аналогии должны жить в `t(tidx(140))`
  и `t(tidx(141))` (соседние блоки, порядок совпадает с порядком в XML:
  pointsScale, valuesScale, seriesScale) -- ГИПОТЕЗА, НЕ проверена ни
  одним семенем. `valuesScale` несёт `showTitle`+`titleArea`+
  `labelFormat` (локализованная строка, не голый цвет); `seriesScale` --
  `titleArea`+`gridLine`+`showInChart`. Оба, вероятно, используют тот же
  `titleArea`-кортеж и `form_chart_scale_title_area_xml`, но проверить
  нужно семенами по одному элементу, ТОЧНО как для `pointsScale` выше
  (control → добавить элемент с default-значениями → добавить по одной
  нестандартной настройке). Готовые семена-скелеты `chart-form-4series-
  src` и метод (см. `pointsScale`'s раздел выше и `git log` коммитов
  `fix(form-chart): decode pointsScale` для точного рецепта) переносятся
  напрямую.
* Полный список elements-тегов записи (для быстрой сверки) --
  `awk 'NR>=6720 && NR<=7030' .../ПроверкаКонтрагента/Forms/Форма/Ext/
  Form.xml | grep -oE "<d4p1:[A-Za-z]+" | sort -u` в дереве
  `$D/cap/ut-r1/src`.

**Вывод**: пункт 4 сузился с "6-8 новых элементов + 2 неопознанные
позиции" до "2 похожих структуры (`valuesScale`/`seriesScale`)". Метод
семян по одному элементу поверх `chart-form-4series` полностью себя
оправдал (9/9 находок
подтвердились с первого-второго семени, ни одной ложной гипотезы, кроме
`pointsScale`'s `labelColor`, которую пришлось отделить от
`gridLinesShowMode`/`gridLine` третьим семенем); продолжать им же.
`chart-form-4series-src` (и его копии `chart-form-*-src` с одним
изменённым элементом каждая) -- готовые скелеты, бери любой и меняй ОДИН
элемент. `valuesScale`, `seriesScale` -- сравнимая по объёму работа для
отдельной волны, но НЕ таких больших размеров, как GanttChart ниже.

## Порядок атаки (рекомендация)

1. Пункт 4, остаток -- семя на `seriesScale`/`valuesScale` по отдельности
   поверх `chart-form-4series` (метод см. выше в разделе про
   `pointsScale`: control → default-элемент → по одной нестандартной
   настройке, дерево `chart-form-pointsscale-src` показывает рецепт --
   скопировать `chart-form-4series-src`, добавить ОДИН элемент в нужную
   XML-позицию между `pointsAxis`/`pointsScale` и `legendPlacement`).
   `pointsScale` (`t(tidx(139))`) уже закрыт и закоммичен
   (`form_chart_scale_title_area_xml` переиспользуем для их
   `titleArea`). Дописать раскладку ДВУХ оставшихся блоков
   (`t(tidx(140))`/`t(tidx(141))`) в `format_form_chart_settings_xml`.
2. Пункты 1-2 (GanttChart) -- самый большой объём, делать последним; начать
   с воспроизведения обёртки `{19,{0,{11},{74,...}}}` и код `chartType`
   `Column3D`, затем идти по XML сверху вниз, member за member, с семенами
   на каждый спорный член (даты -- отдельная проверка формата). Тот же
   payload `{74,...}` внутри -- значит `parse_moxel_chart` (после волны
   2026-08-25) уже умеет `series_count=0`; GanttChart-обёртке нужен только
   свой код `19` и ~55 дополнительных членов поверх него.
