# Дизайн: fast parity evidence

Corpus разделяет три слоя:

1. `seed/` хранит минимальный декларативный вход и provenance Unica.
2. `configuration.cf.b64` и `raw/` фиксируют platform-authenticated physical
   evidence после второго нативного round-trip.
3. `native/` хранит только выбранные эталонные XML/BSL outputs.

Manifest фиксирует точный build 1С, SHA-256 `ibcmd.exe`, версию XML, хэши CF,
raw entries и outputs, а также исключённые volatile files. Правило считается
platform-proven только после двух изолированных импортов и нулевого stable tree
diff. Байты CF не являются parity-критерием: платформа меняет внутреннюю
generation metadata между раундами.

Production decoder не читает manifest. Доказанные enum mappings живут в
`ibcmd-schema`; physical adapter только передаёт raw tokens в fail-closed API.
Unit test включает raw entry и сравнивает весь emitted Task XML с native output.

Unica остаётся hypothesis generator. Её version/commit lineage и DSL digest
обязательны, но platform output имеет больший evidence level. Corpus не заменяет
release-grade evidence из реальной базы и не расширяет release gate #288.
