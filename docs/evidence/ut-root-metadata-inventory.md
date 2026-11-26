# Инвентаризация отсутствующих корневых XML УТ

Дата анализа: 2026-07-23. Это историческая диагностическая выборка, а не
релизное доказательство: новая полная матрица должна быть построена отдельным
воспроизводимым запуском.

## Источник и способ подсчёта

Источник — локальный рабочий каталог `E:\ibcmd_lab\ut`:

- native-oracle находится в Git `fca508289419d3a8ef911e2c7a491da454684861`
  (коммит от 2026-07-22 04:26:41 +03:00);
- рядом находится результат кандидата `ut_ibcmd`;
- в Git-статусе кандидата `D` означает: файл есть в native-oracle, но отсутствует
  после выгрузки кандидатом.

Подсчитывались только пути вида `ut_ibcmd/<коллекция>/<имя>.xml`. Поэтому
вложенные `Ext/Form.xml`, `Ext/Template.xml` и прочие дочерние файлы в это
число не попадают. Команда для воспроизведения (без изменения артефактов):

```powershell
$status = git -C 'E:\ibcmd_lab\ut' -c core.quotepath=false status --porcelain=v1 -uno
$status |
  Where-Object { $_ -like ' D *' } |
  ForEach-Object { $_.Substring(3) } |
  Where-Object { $_ -match '^ut_ibcmd/[^/]+/[^/]+\.xml$' } |
  Group-Object { ($_ -split '/')[1] } |
  Sort-Object Count -Descending
```

## Результат

| Коллекция | Корневой XML отсутствует | Доля |
|---|---:|---:|
| Documents | 282 | 56,5% |
| Catalogs | 198 | 39,7% |
| ChartsOfCharacteristicTypes | 9 | 1,8% |
| BusinessProcesses | 7 | 1,4% |
| DataProcessors | 1 | 0,2% |
| FilterCriteria | 1 | 0,2% |
| Tasks | 1 | 0,2% |
| **Итого** | **499** | **100%** |

Сумма подтверждает прежний total `499`: `282 + 198 + 9 + 7 + 1 + 1 + 1 = 499`.
Наибольший общий кластер — business-object roots `Documents + Catalogs = 480`
(96,2%); крупнейшее одно семейство — `Documents` (282).

## Причина и структурный layout

Во всех семи семействах native XML имеет общий внешний layout:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject ... version="2.20">
    <Kind uuid="...">
        <InternalInfo>...</InternalInfo>
        <Properties>...</Properties>
        <ChildObjects>...</ChildObjects>
    </Kind>
</MetaDataObject>
```

Для крупнейших двух семейств `Kind` равен соответственно `Document` и
`Catalog`. Проверка по одному обезличенному представителю каждого семейства
подтверждает одинаковый контейнер и обязательный блок `InternalInfo` с
`xr:GeneratedType`, затем `Properties`; различается набор свойств и дочерних
объектов.

Причина на уровне имеющихся артефактов одна: **candidate не создал корневой
metadata XML**. Историческая выгрузка не содержит row-level manifest/report с
`object_code`, UUID записи, именем parser-ветки и конкретной причиной
`None`/ошибки, поэтому разделять эти 499 по точному внутреннему отказу
formatter-а было бы недоказуемо. Текущий код подтверждает место отказа:
`extract_metadata_source_xml_from_text_row` вызывает строгие
`parse_document_properties_from_text` и `parse_strict_catalog_properties_from_text`,
а их `None` отменяет создание файла. Это согласуется с отсутствием именно
корней при наличии многих дочерних файлов.

## Предложение общего исправления для первого кластера

Первым закрывать `Catalog` как самостоятельный, повторяемый layout (198
файлов), не добавляя UUID/именных исключений.

1. Вынести строгий путь `parse_strict_catalog_properties_from_text` из
   `Option` в диагностируемый результат: `Parsed`, `Unsupported { stage,
   field_or_ref }`, `Invalid`. В инвентарь корней записывать UUID, object code,
   kind, имя, выбранную ветку и точный stage отказа.
2. Для самого частого `Unsupported` расширить **общий** parser свойств
   `Catalog`: декодировать последовательность `InternalInfo`, общие scalar
   свойства, ссылки/типы и `ChildObjects` из metadata text; форматировать их
   тем же canonical formatter-ом, который уже принимает `CatalogProperties`.
   Не применять fallback с урезанным XML: он не сможет быть byte-identical.
3. Зафиксировать один native/candidate fixture этого layout и тест: parser
   возвращает полный IR, formatter выдаёт полный корневой XML, а инвентарь
   уменьшается ровно на число успешно сформированных Catalog-ов.
4. Затем повторить идентичный диагностический цикл для `Document` (282), где
   уже используется отдельный строгий parser и набор документных свойств.

Такой порядок даёт измеримый, обобщаемый результат и не маскирует отсутствие
XML частичной генерацией.

## Что обязательно должно появиться в свежем manifest/report

Чтобы следующий анализ был доказательным, свежая полная выгрузка `Config`
должна сохранить для **каждой ожидаемой корневой metadata-записи**:

- `file_name`/UUID, `object_code`, определённые `kind`, folder и header name;
- expected relative path и факт его создания;
- `reason` с классификацией: decode failure, unknown kind, missing header,
  missing referenced UUID, parser unsupported/invalid, formatter failure;
- имя parser-ветки и stage/поле отказа без содержимого БД;
- scope (`Full`), `candidate_set_complete`, число expected/written/missing и
  версию платформы/утилит.

Без этих полей исторические 499 можно надёжно группировать только по пути и
native XML layout, но не по первопричине в бинарной metadata-записи.
