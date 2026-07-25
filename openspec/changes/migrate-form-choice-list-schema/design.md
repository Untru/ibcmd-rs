# Дизайн: вертикальный срез формы

Physical decoder извлекает значение и точную provenance, но не знает QName,
XML order или default-emission. Каноническая Form-модель различает отсутствие,
пустое значение, типизированное значение и opaque same-profile payload.

Writer запрашивает exact feature rule по namespace/classifier/feature и целевой
версии. `pending`/`unsupported` rule приводит к типизированной диагностике, а
не к fallback-эвристике. ListSettings делегируется общему DCS serializer.

Переход выполняется узко: сначала ChoiceList и ListSettings, затем удаляются
только доказанно перекрытые ветки старого кода.
