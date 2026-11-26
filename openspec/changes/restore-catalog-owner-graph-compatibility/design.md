# Дизайн

Compact Catalog root не является сокращённым вариантом произвольного owner
graph. Допуск ограничен одновременно family `Catalog`, root arity `2` и
известным диапазоном owner-field count `60..=62`. Дочерние коллекции не
извлекаются эвристически: они создаются пустыми через тот же schema layout и
сохраняют collection provenance.

Физические маркеры layout 56/57, empty reference collection, default input
modes и data-history mode принадлежат `metadata_owner_graph`. MSSQL adapter
получает уже классифицированные enum-варианты и не добавляет новые
name/UUID-special cases.

Layout 57 legacy payload `{0,{2,{0},{1}}}` в слоте, позже используемом для
Characteristics, распознаётся только при полном совпадении структуры и
проецируется в пустую каноническую коллекцию. Любое изменение arity, marker
или owner layout возвращает типизированную ошибку.
