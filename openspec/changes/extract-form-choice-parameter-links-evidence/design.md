# Дизайн: ChoiceParameterLinks writer evidence

Источник — установленный read-only EDT `2025.2.3+30` и локальный inventory
EDT Convector. Extractor вызывает `javap -v -p -c -constants` для точных
классов:

- `FormChoiceParameterLinkWriter`;
- `FormChoiceParameterLinkExtensionWriter`;
- `ExtInfoWriter$ExtInfoFeatureOrderProvider`;
- `FormFeatureNameProvider`, `BaseQNameProvider`, `IXmlElements`;
- `CommonPackageImpl` и `LinkedValueChangeMode`.

Extractor проверяет bundle symbolic name/version, единственность класса,
exact JVM descriptor каждого анализируемого метода, instruction subsequences,
branch targets и constant-pool member references. Входной inventory обязан
быть top-level JSON array.

QName provider chain проверяется целиком:

- runtime module точно возвращает `FormFeatureNameProvider`;
- subclass точно наследует `BaseQNameProvider`, а полный declared-method set
  не содержит override `getElementQName` или capitalization policy;
- `BaseQNameProvider` точно implements `IQNameProvider`, а его constructor
  вызывает virtual package/feature fill methods и сохраняет построенные maps;
- весь 286-instruction `fillSpecifiedFeatureNames` имеет exact envelope и
  состоит из 57 проверенных `ImmutableMap.Builder.put` groups;
- owner feature отсутствует в explicit map, тогда как name/changeMode имеют
  точные mappings;
- base provider содержит точные map lookup, capitalization, package lookup и
  two-argument `QName` calls.

Для regular и extension datapath веток проверяется точный superclass fallback.

Результат — автономный JSON без локальных путей и JAR contents. Каждое
утверждение содержит status и evidence references. Если отсутствует или
неоднозначен release, bundle, метод, control flow, QName provider, model
default, DataPath delegate либо согласование regular/extension, extractor
завершается ошибкой. Свойства вне проверяемого контракта перечисляются в
`notProven`; они не становятся production rules.

Synthetic selftests вызывают основные owner/item/order/extension fact
extractors на положительных instruction/constant-pool fixtures и проверяют
отказы на неверном release, descriptor, нарушенном порядке, missing/ambiguous
instruction, QName, delegate и extension fallback. Детерминизм проверяется
двумя отдельными PowerShell processes, каждый из которых заново читает
inventory, manifests и `javap`.

Name/changeMode model-default fixtures проходят через
`Get-CommonChoiceParameterLinkDefaultFacts` — тот же fail-closed helper,
который вызывает real `Get-ModelFacts`. Negative fixtures заменяют каждый
`aconst_null` и обязаны быть отклонены общим extraction path.
