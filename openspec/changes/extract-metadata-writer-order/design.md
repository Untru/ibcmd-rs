# Дизайн: metadata order corpus

Источник EDT 2025.2.3+30:

- `IMetadataObjectFeatureOrderProvider`;
- `MetadataObjectFeatureOrderProvider`;
- `IProducedTypesOrderProvider`;
- `ProducedTypesOrderProvider`;
- runtime module bindings.

Extractor использует `javap -v -c -p -constants` только в research tool. Он
связывает constant-pool `InvokeDynamic` с точным `BootstrapMethods` method
handle, распознаёт class-to-method map, `ListBuilder.cursor/next` literals,
version branches и static EClass → EReference tables. Неизвестная stack/control
flow форма отклоняется; ordinal pairing не применяется.

Каждая запись содержит provider, section, classifier, ordered feature tokens,
version predicate и evidence. Неожиданный control-flow или nonliteral feature
делает запись `pending`/rejected.

Writer применяет sections только в подтверждённом порядке:

```text
InternalInfo → Properties → ChildObjects
```

Fallback и special cases Configuration/ExchangePlan/ExternalReport/
ExternalDataProcessor представлены явными rules.
