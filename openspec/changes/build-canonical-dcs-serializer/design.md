# Дизайн: DCS canonical layer

Decoder строит ограниченное typed IR для известных settings/schema/template
узлов и сохраняет неизвестные узлы как bounded opaque XML facets с точным
placement. IR не хранит готовые XML-фрагменты для известных полей.

Serializer использует DCS feature semantics и отдельные verified writer rules.
QName, TypeId, picture/color qualification и collection order не выводятся из
входного текста или имени объекта. Один registry обслуживает standalone DCS и
Form ListSettings.

Внедрение начинается с ListSettings и минимального settings document, затем
расширяется на schema/template только при наличии verified evidence.
