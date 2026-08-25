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

### Разведка пункта 4 этой волны (не закрыт -- см. что доказано и что нет)

Слот найден и извлечён напрямую из `1cv8.cf`
(`cf extract 1cv8.cf acd13c5d-edf3-4c18-99d7-663ac866d5e8.0`, форма целиком
-- 946 КБ текста; искомый чарт -- третье по счёту вхождение маркера
`{11},\r\n{74,` в тексте, byte-offset ~674788; НЕ обёрнут второй раз в
`{11},{...}` как у MOXCEL -- сама обёртка это 4-элементный `holder =
{"#", uuid, {11}, {74,...}}`, `data = holder[3]` берётся напрямую).
`realSeriesCount=4` (id 2,3,4,6 -- id 5 не серия, пропущен), `data[]`
length=271, `cursor(tail start)=62`, `tail_len=209`.

**Доказано этой волной (прямым сопоставлением сырых токенов с native XML,
БЕЗ семян -- готовое дерево УТ само стало наблюдением)**:

* Формула `expected_tail_len = 197 + 3*series_count + point_count*(1+4*
  series_count)` из `moxel.rs` (пункт 3) даёт `197+12+0=209` -- ТОЧНОЕ
  совпадение при `series_count=4`, впервые проверено за пределами
  `series_count∈{0,1}`.
* `N = 1+series_count = 5` в списке id шкал (`post[23]='5'`,
  `post[24..29)={0,1,0}..{0,6,0}` -- обратите внимание, id идут `1,2,3,4,6`
  не `1..5`, повторяя пропуск id=5 в самих сериях; `post[29..34)={0,0}`
  пятикратно) -- формула `moxel.rs` подтверждена при N=5.
* `axes_position = 25+2N = 35` (post-относительно) и `rectangle_start =
  63+3*series_count+point_count = 75` -- ОБЕ формулы `moxel.rs`
  подтверждены: `post[75..87)` побайтово совпадает с `elementsChart`
  (0,0,0.17,0), `elementsLegend` (0.1497...,0.9615...,0.0621...,0) и
  `elementsTitle` (0.83,0,0,0.92) из native XML, порядок left,top,right,
  bottom.
* `post[0]/post[1] = "14","2"` (не `"0","0"`) -- `has_extended_scales=false`
  для ЭТОЙ записи, хотя `pointsScale`/`valuesScale`/`seriesScale`
  присутствуют (см. ниже) -- **это ломает пункта-3 гипотезу**: наличие
  `pointsScale`/`valuesScale` НЕ эквивалентно `has_extended_scales` в
  контексте формного атрибута; либо `has_extended_scales`-триггер тут иной,
  либо `post[0]/post[1]` кодируют что-то другое в этом корпусе. Не решено.
* `tail[81]` (FRONT, не post) = `elementsIsInit` -- ПОДТВЕРЖДЕНО семенем
  `chart-no-el-init` (`$D/seeds/chart-src/chartnoelinit`, правит ТОЛЬКО
  `<d3p1:elementsIsInit>` с `true` на `false`): единственный сдвинутый
  токен -- `tail[81]` `"1"→"0"`. Раньше это было зашито как литерал
  `"1"` в `validate_moxel_chart_v74_front` -- корректно для всех текущих
  13+6 примеров (`elementsIsInit` у них везде `true`), но у этой формной
  записи `elementsIsInit=false` -- впервые. НЕ включено в `moxel.rs`,
  т.к. ни один MOXCEL-пример его не использует; пригодится, если найдётся
  такой макет.

**НЕ решено этой волной (пункт 4 всё ещё открыт)**:

* `post[10]` (в OLD/N=2-нумерации -- позиция, которую пункт 3 разметил как
  "устойчиво `0`, причина не найдена") здесь читает `"1"`. Семя
  `chart-no-el-init` ПРОВЕРИЛО и ОПРОВЕРГЛО связь с `elementsIsInit`
  (единственный сдвиг там -- `tail[81]`, `post[10]` не тронут); семя
  `chart-linetype` ПРОВЕРИЛО и ОПРОВЕРГЛО связь с `chartType`
  (единственный сдвиг -- `tail[2]`). Кандидат, не проверенный: `splineMode`
  -- новое поле (`<d4p1:splineMode>SmoothCurve</d4p1:splineMode>`),
  которого НЕТ вообще ни в одном из 13+6 текущих примеров (0 вхождений в
  native XML), и `post[10]` тоже везде `"0"` в этих примерах -- корреляция
  правдоподобна, но всего одно наблюдение, второго значения `splineMode`
  для сравнения нет.
* Пятитокенная проверка перед `elementsChart` (`post[70..75)`) читает
  `"1","1","0","4","8"`, а не `"1","1","1",X,"8"` -- ТРЕТИЙ токен тоже
  флипнулся (`"1"→"0"`), и `X=4`, не `5`/`6` из пункта-3 формулы
  (`isShowLegend=true` здесь, что по пункту-3 давало бы `X=5`). Причина
  не найдена; `elementsIsInit` и `chartType` семенами исключены (те же два
  семя выше это заодно проверили -- ни один не тронул `post[72..75)`).
* Полностью новая лексика XML, для которой в `form_body.rs` нет вообще
  никакого кода (не только неверных предположений, а отсутствия ветки):
  `<d4p1:pointsScale>` (тут -- `titleArea`+`gridLinesShowMode`+`gridLine`+
  `labelColor`, ДРУГАЯ форма, чем у MOXCEL-примера пункта 3, где
  `pointsScale` -- `titleArea`+`labelOrientation`), `<d4p1:seriesScale>`
  (новый верхнеуровневый элемент, `titleArea`? -- не проверено +
  `gridLine`+`showInChart`), `<d4p1:valuesScale>` с `showTitle` (а не
  `labelFormat`/`gridLinesShowMode` из пункта 3), `<d4p1:splineMode>`,
  `<d4p1:valuesToolTipShowMode>`, `<d4p1:pointsDropLinesShowMode>`,
  `<d4p1:valuesDropLinesShowMode>`, `<d4p1:legendPlacement>Bottom</...>`
  (третье значение enum -- было только `None`/`UseCoordinates`).
* Полный список elements-тегов записи (для быстрой сверки) --
  `awk 'NR>=6720 && NR<=7030' .../ПроверкаКонтрагента/Forms/Форма/Ext/
  Form.xml | grep -oE "<d4p1:[A-Za-z]+" | sort -u` в дереве
  `$D/cap/ut-r1/src`.

**Вывод**: пункт 4 -- НЕ точечная правка поверх пункта 3; это отдельный,
сопоставимый по объёму довесок (минимум 6-8 новых элементов/веток плюс
минимум 2 неопознанные позиции `post[10]`/`post[72..75)`), требующий
семян уровня "1 реальная серия" → "4 реальные серии" → каждый новый
XML-элемент по отдельности, ТОЧНО как пункт 3, но на форме, а не на
`CommonTemplate` (метод форменных семян уже доказан
`graphical-schema-field-leftwidest-page`). НЕ начато вслепую в этой волне
умышленно -- слишком много одновременно варьирующихся переменных
(`chartType=Line`, `elementsIsInit=false`, `splineMode` заданный,
`legendPlacement=Bottom`, реальные серии) в единственном природном
примере, чтобы разложить их по отдельности без семян.

## Порядок атаки (рекомендация)

1. Пункт 4 -- начать С СЕМЕНИ, не с прямого разбора: форма на скелете
   `Web_Service` с ОДНИМ атрибутом типа `Chart`, `realSeriesCount=1`,
   ВСЁ остальное -- ровно как один из двух уже доказанных 197-членных
   примеров (не как `ПроверкаКонтрагента`, там сразу 5+ переменных разом).
   Затем по одной переменной: 4 серии, `elementsIsInit=false` отдельно
   (уже частично сделано -- `tail[81]` подтверждён), `splineMode` отдельно,
   `pointsScale`/`seriesScale`/`valuesScale` отдельно, `legendPlacement=
   Bottom` отдельно. Раскладка N-scale-id-list, `axes_position`,
   `rectangle_start` из `moxel.rs`/пункта 3 УЖЕ подтверждены при N=5 --
   не перепроверять, строить поверх них.
2. Пункты 1-2 (GanttChart) -- самый большой объём, делать последним; начать
   с воспроизведения обёртки `{19,{0,{11},{74,...}}}` и код `chartType`
   `Column3D`, затем идти по XML сверху вниз, member за member, с семенами
   на каждый спорный член (даты -- отдельная проверка формата). Тот же
   payload `{74,...}` внутри -- значит `parse_moxel_chart` (после волны
   2026-08-25) уже умеет `series_count=0`; GanttChart-обёртке нужен только
   свой код `19` и ~55 дополнительных членов поверх него.
