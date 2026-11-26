# Gantt chart with four time scale levels

Evidence from the local Src corpus, report `узПланированиеПроекта`, template
`пмГант_ГантДиаграмма`, exported by 1C 8.3.27.2214.

`object-payload.txt` is the stored version-19 chart object, extracted from
template storage `5328a37d-ea68-4d87-8707-1a7e55aa9b5b.0` without changing
its values. `object.xml` is the exact native `<object>` fragment.

The time scale has four levels, mixed solid/dotted lines, a localized month
format, and current level 1. The background interval has 864000000 ticks.
The regression test compares the complete object bytes against the native XML.
