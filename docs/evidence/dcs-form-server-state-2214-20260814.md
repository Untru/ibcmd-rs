# Clean-room UniversalListServerOnlyState — 8.3.27.2214 / XML 2.20 (2026-08-14)

Пятая лабораторная сессия Parallels «Windows 11» закрыла evidence-часть
DCS-FORM-SERVERSTATE-01 (PRD WS5). Authority: 1С `8.3.27.2214`, `ibcmd.exe` SHA-256
`11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b`, locale `ru_RU`.
Протокол batch-стандартный; seed-форма собрана исключительно из cross-evidence
базового корпуса `dcs-form-attributes-conditional-appearance` с протоколом
происхождения каждой части.

## Новые корпуса

| Корпус | Проба | Состав |
|---|---|---|
| `dcs-form-dynamic-list-server-state` | F1 | Catalog `CorpusList` + списочная форма с DynamicList (ManualQuery=false, MainTable по каталогу), без ListSettings |
| `dcs-form-list-settings-server-state` | F2 | То же + явные ListSettings (фильтр по строковому полю, order Asc) |

Retained каждой: round-2 CF, native Form.xml, raw form body (packed/unpacked) и
раскодированный ServerState-конверт.

## Доказанные факты

1. **Полный побайтовый детерминизм** обеих проб между двумя независимыми свежими
   ИБ: native-дерево целиком (кроме ConfigDumpInfo.xml), Form.xml, raw form body.
   Провенанс-политика для ServerState не требуется — байтовое равенство валидно.
2. **ServerState отсутствует в native Form.xml** (`ibcmd config export` его не
   эмитит) и при этом **безусловно присутствует в raw storage** как property-bag
   ключ `ServerState`: chunk-encoded (magic `0x41 0xC1`, короткий первый чанк),
   формат байт-в-байт совпадает с ожиданиями существующего
   `decode_form_server_state_chunks`.
3. **Содержимое — пустой самозакрывающийся wrapper**
   `<UniversalListServerOnlyState xmlns="" …/>`, идентичный (одинаковый SHA-256)
   для F1 и F2: явные ListSettings конверт НЕ наполняют. ServerState — sibling
   Filter/Order/AutoSaveUserSettings в плоском property bag, не вложен в
   ListSettings.

## Незаявленное (рабочая гипотеза, не доказана)

Наполнение конверта, вероятно, требует реального клиентского выполнения списка;
для конфигурационного транспорта (config import/export) конверт всегда пуст.
Гипотеза не является policy и не даёт права на emission непустых состояний.

## Границы

Семантика непустого ServerState не доказана и не моделируется. compiler_acceptance
корпусов отложен. Формы вне DynamicList не расширяются (граница WS5).

## Cleanup

Guest-каталог `C:\lab-fprobes` удалён (C: 119,8 → 121,7 ГБ), VM остановлена,
основной чекаут лабораторной сессией не изменялся.
