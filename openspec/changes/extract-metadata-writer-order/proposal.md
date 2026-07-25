# Предложение: извлечь metadata writer order providers EDT

## Зачем

Metadata XML order сейчас частично задаётся локальными property schemas.
EDT содержит отдельные `MetadataObjectFeatureOrderProvider` и
`ProducedTypesOrderProvider`, которые формируют section/order с учётом версии.

## Что меняется

- Research extractor получает derived order tables из exact provider bytecode.
- Corpus различает sections: innerInfo, properties, children, producedTypes.
- Version predicates и fallback rules фиксируются отдельно.
- Metadata writer получает schema API, не читая EDT/JAR в production.

## Не входит

QName, default/nil/empty и compatibility rules не объявляются из order provider:
для них требуется отдельный анализ `MetadataObjectWriter` и smart writers.
