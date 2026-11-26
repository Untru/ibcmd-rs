# Предложение: извлечь evidence ChoiceParameterLinks из EDT

## Зачем

Текущая реализация формы содержит локальные знания о
`InputFieldExtInfo.choiceParameterLinks`, но они не отделены от догадок и не
имеют автономного writer-derived evidence. Для безопасной будущей миграции
нужен узкий read-only extractor точного EDT release.

## Что меняется

- Research extractor проверяет exact release и bundle coordinates EDT
  `2025.2.3+30`.
- Каждый анализируемый method block принимается только с exact JVM descriptor.
- QName считается доказанным только при полном runtime binding → subclass →
  base fallback chain, включая отсутствие relevant override и полный feature
  map.
- Extractor доказывает owner wrapper, item QName, empty/null/version/order,
  порядок полей `FormChoiceParameterLink`, QName/default/lexical map полей,
  DataPath delegate и extension behaviour.
- В репозиторий переносится только очищенный deterministic JSON и synthetic
  `javap` selftest evidence.
- Недоказанные свойства публикуются как explicit `not-proven`, а не как
  production rule.
- Детерминизм проверяется двумя независимыми extraction processes.

## Не входит

- Изменение production form writer.
- Изменение coverage или policy baseline.
- Перенос JAR/class payload, локальных путей EDT или machine-specific
  inventory.
- Вывод production emission rule из неполного или неоднозначного evidence.
