# Дизайн: полный Xcore corpus

Importer остаётся research-only и читает Xcore непосредственно из inventory
selected JAR. Расширение выполняется evidence-first:

1. запустить текущий fail-closed parser на `Bundle=*`, `Scope=model/*.xcore`;
2. кластеризовать rejected syntax;
3. добавлять grammar production только по реальному counterexample;
4. для каждой новой production добавить negative fixture;
5. сгенерировать corpus и reject report с устойчивой сортировкой.

Corpus хранит только local declared features. Inheritance expansion выполняется
отдельным canonical coverage layer, чтобы не терять ownership и provenance.

Definition of completeness:

- все выбранные resources перечислены в processed/rejected summary;
- processed + rejected = selected;
- повторный запуск имеет тот же SHA-256;
- committed output не содержит vendor source text, binaries или absolute paths.
