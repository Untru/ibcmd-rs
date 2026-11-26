# Однополевые пробы Configuration Properties и повторная приёмка A6 — 8.3.27.2214 (2026-08-14)

Седьмая лабораторная сессия Parallels «Windows 11». Authority: 1С `8.3.27.2214`,
`ibcmd.exe` SHA-256 `11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b`,
locale `ru_RU`. Протокол batch-стандартный; все принятые пробы — две независимые
свежие ИБ, полный детерминизм (footer-noise исключён как в прошлых сессиях).

## Карта «смещение кортежа → свойство» (закрыта для шести полей)

Пять однополевых корпусов `configuration-property-*` дают прямую атрибуцию;
шестое поле привязано строгим исключением из замкнутой тройки CP2:

| Смещение | Свойство | Атрибуция |
|---|---|---|
| 428 | IncludeHelpInContents | прямая (SP1) |
| 623 | UseManagedFormInOrdinaryApplication | прямая (SP2) |
| 625 | UseOrdinaryFormInManagedApplication | прямая (SP3) |
| 867 | ModalityUseMode | прямая (SP5) |
| 906 | InterfaceCompatibilityMode | прямая (SP6) |
| 2669 | SynchronousPlatformExtensionAndAddInCallUseMode | исключение из замкнутой группы CP2 (две из трёх привязаны прямо) |

SP4 (DataLockControlMode=Managed) оказался no-op: Managed уже default базовой
конфигурации — свойство остаётся нелокализованным до пробы с настоящим
non-default значением. Не пробовались вовсе: ObjectAutonumerationMode,
MainClientApplicationWindowMode.

## A6 (dcs-area-style-item-uuid): новый различимый исход

Resolver-компиляция (eb045ac) прошла: кандидат собран offline, наложен в retained
CF, **принят платформой** на `load --apply` и сохранён бит-в-бит (`config save`).
Однако `config export` платформы завершился ошибкой дословно:

```
Cannot perform the operation: errors occurred while exporting the configuration to XML files. Stream format error
```

`Template.xml` записан нулевой длины; все остальные объекты (Configuration.xml,
StyleItems/CorpusAccent.xml) экспортированы побайтово равными retained-эталонам.
Контрольный прогон немодифицированного базового CF экспортируется чисто —
расхождение локализовано в скомпилированном теле кандидата: import его прощает,
export — нет. Причина не диагностировалась на VM (ничего не угадывалось);
оффлайн-дифф кандидата против retained raw-unpacked — отдельная задача.
`compiler_acceptance` для A6 по-прежнему не заявляется.

## Cleanup

Guest-каталог `C:\lab7` удалён, VM остановлена (после самопроизвольного suspend —
возобновлена и остановлена штатно). Обнаружены и переданы в план следующей сессии
посторонние каталоги от бага интерполяции ранних сессий (`C:\lab-batch$coord$r`,
`C:\lab-chips${probe}`). Основной чекаут лабораторной сессией не изменялся.
