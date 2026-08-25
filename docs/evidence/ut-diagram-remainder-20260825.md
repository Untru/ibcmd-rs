# Остаток по диаграммам УТ после волны Bots/GraphicalSchemaField/Task

Статус после волны 2026-08-25 (вторая половина дня): паритет УТ поднят с 7
расходящихся + 1 невыданный до **3 расходящихся** (было 4). Bots, Tasks, оба
GraphicalSchemaField и пункт 3 ниже (`СравнительныйАнализМенеджеров`)
закрыты (см. коммиты `596bcc8`, `eaa6847`, `6778b19`, `7982d6c`, `292d807` и
фикстуры `tests/fixtures/native-evidence/8.3.27.2214/
{bot-predefined-picture,task-number-allowed-length-data-lock-mode,
graphical-schema-field-leftwidest-page,moxel-chart-series-count-zero}`).

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

Структура найдена частично (сырые байты обеих записей были в
`$D/ut-agent-priv/gantt1`, `gantt2` в этой волне, не сохранены как
фикстура -- переснять через `cf extract <cf> <uuid>.0` по storage-ключам из
`report.json`, uuid объектов -- `beb44b02-5d5e-4127-aee7-e184a43a4210` и
`3c2c192e-4869-4cc6-ba47-1fadac5b9678`):

```
fields[12] = {19, {0, {11}, {74, ...обычный chart payload...}}}
```

То есть `object`-слот GanttChart -- это НЕ прямой `{11},{74,...}` (как у
обычного Chart), а обёртка кода `19` вокруг тройки `{0, {11}, {74,...}}`;
сам `{74,...}` -- **тот же payload**, что `parse_moxel_chart` уже умеет
разбирать (seriesCurId/pointsCurId/isSeriesDesign/realSeriesCount/...,
включая случай 0 серий: обе записи тоже пустые, `realSeriesCount=0`,
`realPointCount=0`, `curSeries=-1`). `chartType` у одной записи -- код,
дающий `Column3D` (нативно: `<d3p1:chartType>Column3D</d3p1:chartType>`) --
**новый код**, не входящий в текущую таблицу `moxel_chart_type` (`0`/`9`/
`38`); нужно вычислить сырое значение (`tail.get(2)`) и добавить строку.

Нативный XML доказывает: `<object xsi:type="d3p1:GanttChart">` = ровно
`<d3p1:chart>` (все поля обычного Chart, 1:1 с `<object xsi:type=
"d3p1:Chart">`), а ЗА ним ещё **~55 дополнительных элементов**, специфичных
для Ганта: `<d3p1:points>`, `<d3p1:series>` (each -- "cache template" с
testMode/value/contentCacheItem), `<d3p1:drawEmpty>`, `<d3p1:timeScale>`
(с вложенным `<d3p1:level>` -- measure/interval/show/line/scaleColor/...),
`<d3p1:keepScaleVariant>`, `<d3p1:fixedVariantMeasure/Interval>`,
`<d3p1:autoFullInterval>`, `<d3p1:fullIntervalBegin/End>` (ДАТЫ -- ещё один
формат для разбора), `<d3p1:visualBegin>`, `<d3p1:intervalDrawType>`,
`<d3p1:backIntervals>`, `<d3p1:linksColor/Line>`, `<d3p1:showPointsText>`,
`<d3p1:showData>`, `<d3p1:textPlacement>`, `<d3p1:intervalTextRepresentation>`
и другие -- смотри полный список прямо в `$D/cap/ut-r1/src/Reports/
АнализЖурналаРегистрации/Templates/.../Ext/Template.xml`, строки 2939-3039
(после `</d3p1:chart>` до `</object>`).

**Объём**: это отдельный, большой декодер (~55 членов, включая вложенные
структуры и минимум один новый формат даты), а не точечная правка. Всего
2 нативных примера в УТ -- по доктрине этого мало для 200-членной записи;
нужны семена с вариациями (пустой Гант уже есть -- оба текущих примера
пустые; нужен минимум один НЕпустой семпл, если такой найдётся в
sslbase/ssl/uh, и/или семена с рукописным `Ext/Template.xml`, аналогично
плану для пункта 3 выше).

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
новый с нуля. Дополнительно неясно, ЯВЛЯЕТСЯ ли `ПроверкаКонтрагента/Forms/
Форма`'s `ДиаграммаПоказателей` формным АТРИБУТОМ (как оба доказанных
примера) или именно `ChartField`-полем формы (см. `FORM_DOCUMENT_FIELD_GEOMETRY`
в `form_body.rs`, запись `"ChartField"`, geometry-only, не про `<Settings>`)
-- эти два пути могут оказаться одним и тем же слотом или разными; не
проверено.

**Метод пункта 3 переносится сюда напрямую**: семя на скелете `Web_Service`
с вручную заданными 4 сериями (скопировать `<d4p1:realSeriesData>` из
рабочего MOXCEL-примера, см. `docs/evidence/seed-configurations-method.md`
и фикстуру `moxel-chart-series-count-zero` этой волны как образец приёма
"одно изменение XML -- один семя -- один raw-diff").

## Порядок атаки (рекомендация)

1. Пункт 4 (`ChartField`/атрибут формы `ПроверкаКонтрагента/Forms/Форма`) --
   расширить `parse_form_chart_settings_xml`/`format_form_chart_settings_xml`
   в `form_body.rs` на `realSeriesCount > 0`: семя с 1, затем 4 реальными
   сериями на скелете `Web_Service`, сравнить сырые байты с 197-членным
   доказанным хвостом (аналогично тому, как пункт 3 нашёл `has_extended_scales`
   и `is_title_init` в `moxel.rs`). Если раскладка окажется той же, что
   `parse_moxel_chart`'s (payload идентичен), можно опереться на готовые
   формулы `moxel.rs` (`N = 1 + series_count` и т.д.) как отправную точку.
2. Пункты 1-2 (GanttChart) -- самый большой объём, делать последним; начать
   с воспроизведения обёртки `{19,{0,{11},{74,...}}}` и код `chartType`
   `Column3D`, затем идти по XML сверху вниз, member за member, с семенами
   на каждый спорный член (даты -- отдельная проверка формата). Тот же
   payload `{74,...}` внутри -- значит `parse_moxel_chart` (после волны
   2026-08-25) уже умеет `series_count=0`; GanttChart-обёртке нужен только
   свой код `19` и ~55 дополнительных членов поверх него.
