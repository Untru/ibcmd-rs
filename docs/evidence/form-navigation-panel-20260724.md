# NavigationPanel: evidence для #242

Статус: **PASS**. Анализатор читает входные выгрузки и raw-layouts; изменяет только явно заданные файлы evidence.

## Входы

| Набор | Run ID / commit | Путь |
|---|---|---|
| Native | `ut_ibcmd_full_cfef6ee7_20260723_121002_ut` | `E:\ibcmd_lab\parity\ut_ibcmd_full_cfef6ee7_20260723_121002_ut\native` |
| Candidate до исправления | `ut_full_87c8dc3_20260723_195521` | `E:\ibcmd_lab\ut_full_87c8dc3_20260723_195521` |
| Candidate после исправления (выборочный) | `ut_nav_3713ec36_20260724_selected_after` / `3713ec36c4c8fc965b63219e28fb405bc9d1d093` | `E:\ibcmd_lab\parity\ut_nav_3713ec36_20260724_selected_after` |
| Canonical diff | SHA256 `3dd8af43b270402a1ac80e7cb09288b18db878ae705dbfcd3b8c34ee8f7d0251` | `E:\ibcmd_lab\ut_full_87c8dc3_20260723_195521_canonical_diff.json` |
| Raw probes | 4 файла | `E:\ibcmd_lab\parity\ut_nav_raw_probe_3713ec36_20260724` |

## Измерения

- Изменённых пар Form.xml: **2569**.
- Left-only сигнатур `Form/CommandInterface/NavigationPanel/Item`: **209** в **57** файлах.
- Реально отсутствующих команд по мультимножеству `Command`: **211** в **57** файлах с left-only Item.

## Классификация raw-layout

| Kind | Класс | Количество |
|---:|---|---:|
| 0 | `standard-or-object-reference` | 3 |
| 1 | `register-open-by-recorder` | 6 |
| 3 | `open-by-value-or-command` | 4 |
| 4 | `catalog-or-register-open-by-value` | 2 |
| 5 | `register-open-by-value` | 2 |

Проверено **17** элементов. Неизвестных, битых или неучтённых вариантов нет.

## Выборочная проверка после исправления

| Относительный путь | Состояние | Native, bytes / SHA256 | After, bytes / SHA256 |
|---|---|---|---|
| `Catalogs/ВЕТИСПрисоединенныеФайлы/Forms/ФормаЭлемента/Ext/Form.xml` | совпадает побайтно | 665 / `b95b504fb3a41d54e25b6b764d887064bd740018ac7bfb4d9ea4203537ada9d8` | 665 / `b95b504fb3a41d54e25b6b764d887064bd740018ac7bfb4d9ea4203537ada9d8` |
| `Catalogs/ДоговорыМеждуОрганизациями/Forms/ФормаЭлемента/Ext/Form.xml` | совпадает побайтно | 491 / `873014a637ef69561f01466edbdd95f2d9291b8ae9f7a11d975403ae093678a1` | 491 / `873014a637ef69561f01466edbdd95f2d9291b8ae9f7a11d975403ae093678a1` |
| `Documents/АктВыполненныхРабот/Forms/ФормаДокумента/Ext/Form.xml` | совпадает побайтно | 2214 / `21dcab7268bb00f42100a6ea5ab4b2eba43c7b5f080e5d4c034557e16dff3abb` | 2214 / `21dcab7268bb00f42100a6ea5ab4b2eba43c7b5f080e5d4c034557e16dff3abb` |
| `Catalogs/ВЕТИСПрисоединенныеФайлы/Forms/ФормаОшибкиСтрокой/Ext/Form.xml` | отсутствует в обоих | 0 / — | 0 / — |

Сохранены только относительные пути, длины и SHA256 фрагментов; XML и UUID объектов в evidence не включены.

## Воспроизведение

```powershell
$selected = @('Catalogs/ВЕТИСПрисоединенныеФайлы/Forms/ФормаЭлемента/Ext/Form.xml', 'Catalogs/ДоговорыМеждуОрганизациями/Forms/ФормаЭлемента/Ext/Form.xml', 'Documents/АктВыполненныхРабот/Forms/ФормаДокумента/Ext/Form.xml', 'Catalogs/ВЕТИСПрисоединенныеФайлы/Forms/ФормаОшибкиСтрокой/Ext/Form.xml')
$rawRoots = @('E:\ibcmd_lab\parity\ut_nav_raw_probe_3713ec36_20260724')
& .\scripts\measure-form-navigation-parity.ps1 `
  -CanonicalDiffPath 'E:\ibcmd_lab\ut_full_87c8dc3_20260723_195521_canonical_diff.json' `
  -NativeRoot 'E:\ibcmd_lab\parity\ut_ibcmd_full_cfef6ee7_20260723_121002_ut\native' `
  -BaselineCandidateRoot 'E:\ibcmd_lab\ut_full_87c8dc3_20260723_195521' `
  -AfterCandidateRoot 'E:\ibcmd_lab\parity\ut_nav_3713ec36_20260724_selected_after' `
  -NativeRunId 'ut_ibcmd_full_cfef6ee7_20260723_121002_ut' `
  -BaselineRunId 'ut_full_87c8dc3_20260723_195521' `
  -AfterRunId 'ut_nav_3713ec36_20260724_selected_after' `
  -AfterCommit '3713ec36c4c8fc965b63219e28fb405bc9d1d093' `
  -SelectedPath $selected -RawLayoutRoot $rawRoots `
  -OutputJson '.\docs\evidence\form-navigation-panel-20260724.json' `
  -OutputMarkdown '.\docs\evidence\form-navigation-panel-20260724.md'
```

Команда пересчитывает оба счётчика и evidence. Для проверки уже зафиксированных файлов добавьте ``-VerifyOnly``; любое расхождение или необработанный raw-элемент завершает процесс с ненулевым кодом.
