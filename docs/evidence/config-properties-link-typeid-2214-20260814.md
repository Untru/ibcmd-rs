# Пробы Configuration Properties и link+TypeId — 8.3.27.2214 / XML 2.20 (2026-08-14)

Шестая лабораторная сессия Parallels «Windows 11». Authority: 1С `8.3.27.2214`,
`ibcmd.exe` SHA-256 `11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b`,
locale `ru_RU`. Протокол batch-стандартный, две независимые свежие ИБ на пробу; все
принятые пробы детерминированы побайтово, native re-export равен seed у всех трёх.

## Новые корпуса

| Корпус | Назначение | Факты |
|---|---|---|
| `dcs-query-union-link-typeid` | Пришпиливание фикса гейта normalizer (провал inner-schema + непустой type_index случайно отсекает query-union-link fallback) | Перенос доказанной TypeId-конструкции из `dcs-typeid-reference` в query-union-link схему принят платформой; Template.xml равен seed |
| `configuration-properties-boolean-group` | Позиции булевых свойств Properties | Три булевых non-default свойства → изолированные однобайтовые замены в config-body по смещениям 428, 623, 625 |
| `configuration-properties-enum-group` | Позиции enum-свойств Properties | Три enum non-default значения → смещения 867, 906, 2669 |

Соответствие «смещение → свойство» пока **групповое** (каждая проба меняла три
свойства одновременно): индивидуальная привязка требует однополевых проб — это
остаток задачи Configuration Properties mapping. Также идентифицировано и исключено
из анализа межэкземплярное «шумовое» поле-футер (trailing signed int32 вне кортежа
Properties).

## Заблокированная проба

CP3 (reference-группа с Role/DefaultRoles) — BLOCKED-SHAPE: во всех
`tests/fixtures/` нет ни одного Rights/Role-образца; форма не изобреталась.

## Cleanup

Guest-каталог `C:\lab-chips` удалён (C: 118,4 → 121,4 ГБ), VM остановлена,
основной чекаут лабораторной сессией не изменялся.
