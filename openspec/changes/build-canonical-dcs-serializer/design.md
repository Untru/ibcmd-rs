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

Source-owned delegation не является вторым сериализатором: неизменённое
поддерево остаётся у одного доказанного physical owner. Его мутация, перенос в
другой wrapper или cross-profile emission запрещены до отдельного writer rule.
