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
делегируются вариантам позиционно. Это evidence для framing и source-owned
placement, но не готовая поддержка нескольких вариантов reverse compiler и не
полная typed schema model.

Source-owned delegation не является вторым сериализатором: неизменённое
поддерево остаётся у одного доказанного physical owner. Его мутация, перенос в
другой wrapper или cross-profile emission запрещены до отдельного writer rule.
