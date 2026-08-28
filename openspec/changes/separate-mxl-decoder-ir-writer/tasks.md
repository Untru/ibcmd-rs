# Задачи: MXL decoder / IR / writer separation

- [x] Зафиксировать scope: только MOXCEL template source assets.
- [x] Ввести typed canonical hand-off с palette provenance и явным format map.
- [x] Ввести стабильные diagnostics, различающие decoder и writer.
- [x] Перевести production MXL extraction на hand-off, не меняя QName/order.
- [x] Добавить unit tests для map и write-plan diagnostic contracts.
- [ ] Добавить новый XML feature только с отдельным fixture/evidence.
- [ ] Удалить test-only legacy helpers после полного migration всех callers.
