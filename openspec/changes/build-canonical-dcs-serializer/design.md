# Дизайн: DCS canonical layer

Decoder строит ограниченное typed IR для известных settings/schema/template
узлов. Ветви, которые подтверждены профилем, но ещё не типизированы canonical
IR, могут оставаться bounded source-owned только по отдельному positive rule с
точным placement и provenance. Действительно неизвестный профилю QName не
считается opaque автоматически: он отклоняется fail-closed. IR не хранит
готовые XML-фрагменты для известных полей.

Serializer использует DCS feature semantics и отдельные verified writer rules.
QName, TypeId, picture/color qualification и collection order не выводятся из
входного текста или имени объекта. Один registry обслуживает standalone DCS и
Form ListSettings.

Внедрение начинается с ListSettings и минимального settings document, затем
расширяется на schema/template только при наличии verified evidence.

Физический schema/template envelope отдельно подтверждён на одном и двух
прямых root `settingsVariant`: поле `u32` по смещению 4 является числом внешних
`Settings`, за ним следуют `settings_count + 1` длин `u64`, а settings documents
делегируются вариантам позиционно. Bounded reverse compiler для одного-двух
вариантов использует этот общий binder. Первый typed inner-schema cohort также
общий: Local/Object simple one-string-field и rich string/decimal cohorts,
calculated field, ungrouped totals, scalar parameter и variant shells проходят через canonical IR и
evidence-gated XML codec. Один exact current-config reference coordinate также
типизирован: `CatalogRef.FilterProbe` ↔ platform storage `TypeId`; другие
reference/type families, Query/Union/link, AreaTemplate и cardinality больше
двух остаются unsupported до отдельных правил.

Source-owned delegation не является вторым сериализатором: неизменённое
поддерево остаётся у одного доказанного physical owner. Его мутация, перенос в
другой wrapper или cross-profile emission запрещены до отдельного writer rule.
