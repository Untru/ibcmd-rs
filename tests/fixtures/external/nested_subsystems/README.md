# Nested subsystems (#417)

`source/` is `tests/fixtures/file-infobase/tiny-source` with a subsystem
`Верх` whose child subsystem `Низ` holds the common module
`ФайловаяБазаПроверка` in its content. `input.cf` is what 1C:Enterprise
8.5.1.1529 (Windows) saves of it:

```
ibcmd infobase create --db-path=db --create-database --import=source --apply --force
ibcmd infobase config save --db-path=db input.cf
```
