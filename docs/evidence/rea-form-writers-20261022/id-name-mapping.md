# ID атрибутов → имена свойств: сопоставление через EDT

> **Устарело.** Этот способ опирался на порядок свойств в EDT, но формат EDT не
> равен формату конфигуратора. Сверка с XDTO-моделью показала 13 расхождений
> из 30 у `UsualGroup` и 89 из 89 у `Table`. Актуальный источник имён —
> [../rea-xdto-20261022/README.md](../rea-xdto-20261022/README.md): пакет
> `logform` из `mngbase_root.res`, где ID — позиция свойства в цепочке
> наследования. Файлы `id-name-*.json` оставлены как история и для кода не
> годятся.

Дата: 22.10.2026. Ветка: `feat/rea-form-property-inventory-20261022`.

## Проблема

Писатель платформы пишет атрибуты по числовым ID (`0x1f`, `0x20`, …) в
фиксированном порядке и с пропуском умолчаний. ID без имён бесполезны для
кода `ibcmd-rs`. Реестра `{ID → имя}` в бинарниках платформы найти не удалось.

## Решение

EDT-плагин `com._1c.g5.v8.dt.form.export.xml` содержит тот же порядок свойств
в виде списков EMF-свойств с условиями по версии платформы:

```java
builder.add(FormPackage.Literals.USUAL_GROUP_EXT_INFO__GROUP);
builder.add(FormPackage.Literals.USUAL_GROUP_EXT_INFO__CHILDREN_ALIGN);
...
if (version.compareTo(Version.V8_5_1) >= 0) {
    builder.add(FormPackage.Literals.USUAL_GROUP_EXT_INFO__COLLAPSED_REPRESENTATION_ITEM);
}
```

Списки EDT и последовательности ID платформы совпадают по длине, поэтому
позиция даёт имя для ID. Инструмент — `tools/rea/edt_feature_order.py`:

```powershell
python tools/rea/edt_feature_order.py `
  --java C:/Tools/edt-form-export/src/.../ExtInfoWriter.java `
  --native docs/evidence/rea-form-writers-20261022/8.5.1.1529.json `
  --method getUsualGroupExtInfoFeatures --element UsualGroup --version 8.5.1 `
  --out docs/evidence/rea-form-writers-20261022/id-name-8.5.1-UsualGroup.json
```

Скрипт вычисляет условия `version.compareTo(...)` и `version.isLessThan(...)`
для целевой версии, собирает порядок и склеивает его с ID; при несовпадении
длин возвращает ошибку.

## Проверено

| Элемент | Платформа 8.5.1 | EDT 8.5.1 | Совпало |
|---|---|---|---|
| UsualGroup | 30 | 30 | да |
| CommandBar | 3 | 3 | да |
| Pages | 3 | 3 | да |
| ColumnGroup | 12 | 12 | да |
| Page | 16 | 14 | нет |
| Table | 89 | 98 | нет |
| Button | 49 | 47 | нет |

Для `UsualGroup` совпадает и 8.3.27: 22 ID против 22 свойств EDT.

## Пример: UsualGroup 8.5.1

| ID | feature EDT | XML |
|---|---|---|
| `0x1f` | `USUAL_GROUP_EXT_INFO__GROUP` | `group` |
| `0x20` | `...__CHILDREN_ALIGN` | `childrenAlign` |
| `0x21` | `...__HORIZONTAL_SPACING` | `horizontalSpacing` |
| `0x22` | `...__VERTICAL_SPACING` | `verticalSpacing` |
| `0x23` | `...__HORIZONTAL_ALIGN` | `horizontalAlign` |
| `0x24` | `...__VERTICAL_ALIGN` | `verticalAlign` |
| `0x25` | `...__BEHAVIOR` | `behavior` |
| `0x26` | `...__COLLAPSED_REPRESENTATION_TITLE` | `collapsedRepresentationTitle` |

Полностью — [id-name-8.5.1-UsualGroup.json](id-name-8.5.1-UsualGroup.json).

## Важное следствие

В 8.3.27 последовательность начинается с `0x1d`, в 8.5.1 — с `0x1f`. Это не
удаление свойств `0x1d/0x1e`, а сдвиг нумерации из-за добавленных в 8.5
свойств. При адаптации нельзя переносить ID между версиями — нужно
перегенерировать сопоставление.

## Table: кандидат без валидации

`getTableFeatures` даёт 99 свойств; последние 10 — контейнеры и дочерние
элементы (`rowFilter`, `contextMenu`, `autoCommandBar`, `extendedTooltip`,
три `*Addition`, `handlers`, `items`, `extInfo`). После их отбрасывания
получается 89 — ровно столько атрибутов пишет нативный писатель таблицы.
Выравнивание по позиции даёт правдоподобную карту (`0xc` → `representation`,
`0x51` → `autoFill`, `0x4e` → `enabled`), но подтвердить её на реальной
выгрузке не удалось: в доступных конфигурациях (ServiceDesk, УХА, EDT-проекты)
у таблиц заполнены только `name` и `id`.

Поэтому [id-name-8.5.1-Table.json](id-name-8.5.1-Table.json) помечен как
кандидат: перед использованием нужна сверка с выгрузкой формы, где у таблицы
заданы ширина, заголовок и режимы выбора.

## Что дальше

`Table`, `Button`, `Field`, `Form` пишутся комбинированным списком (общие
свойства элемента + специфичные), поэтому их длины не совпадают с одним
методом EDT. Нужно собрать композицию списков (`FormItemWriter.get*` +
`FormTableWriter` и т. п.) и снова сверить длины.
