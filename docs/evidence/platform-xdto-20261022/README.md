# XDTO-пакеты платформы 8.5.1.1529

Извлечены из `*_root.res` скриптом
`tools/rea/extract_xdto_packages.py` (пакеты лежат в ресурсах открытым
текстом, `<package targetNamespace=...>`).

## Итог

| Показатель | Значение |
|---|---|
| Пакетов | 89 |
| Namespaces | 89 (дубликатов нет) |
| Суммарный объём | 1 714 413 байт |

Манифест: [manifest.json](manifest.json) — ресурс, namespace, файл, размер,
число `objectType` и `property`.

## Крупнейшие и важные пакеты

| Namespace | objectType | property |
|---|---|---|
| `http://v8.1c.ru/8.3/MDClasses` | 186 | 1839 |
| `http://v8.1c.ru/8.3/xcf/logform` | — | — (извлечён ранее, 66/68 типов) |
| `http://v8.1c.ru/8.3/xcf/readable` | 32 | 104 |
| `http://v8.1c.ru/8.3/xcf/extrnprops` | 33 | 86 |
| `http://v8.1c.ru/eventLog` | 127 | 60 |

## Зачем

Это дешёвый и полный источник фактов о формате: типы, свойства, умолчания,
формы (атрибут/элемент). Для оставшихся задач (компилятор EDS, MXL) схемы
дают карту того, что вообще может быть записано, без декомпиляции.

## Как повторить

```powershell
python tools/rea/extract_xdto_packages.py `
  --bin "C:\Program Files\1cv8\8.5.1.1529\bin" `
  --out D:\temp\xdto-all `
  --manifest docs/evidence/platform-xdto-20261022/manifest.json
```
