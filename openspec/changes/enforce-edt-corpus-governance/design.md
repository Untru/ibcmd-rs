# Дизайн: corpus governance gate

Validator работает без EDT и проверяет repository tree:

- запрещённые extensions/signatures: jar, class, dll, so, dylib и Xcore source;
- portable paths и file URIs;
- versioned source metadata;
- verified evidence с непустым source;
- согласованность summary и schema validation через `ibcmd-schema`;
- allowlist только для очищенных JSON и research scripts.

CI вызывает validator в offline workflow. Проверка не читает пользовательские
УТ/БСП fixtures и не публикует прикладные данные.
