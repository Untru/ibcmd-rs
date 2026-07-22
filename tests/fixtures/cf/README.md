# Offline CF fixture corpus

Этот каталог содержит только специально созданные clean-room fixtures без прикладного кода, секретов и компонентов 1С. Бинарные данные хранятся как RFC 4648 Base64 (`*.cf.b64`), поэтому Git показывает каждое изменение; тест декодирует их только в памяти и проверяет SHA-256 из `manifest.json`.

## Что именно зафиксировано

- `format15-clean-room.cf.b64`: 32-битные addresses, 31-байтовые block headers и sentinel `0x7fffffff`.
- `format16-clean-room.cf.b64`: semantic Format15 envelope ровно `0x1359` байт и основной контейнер с 64-битными addresses, 55-байтовыми block headers и sentinel `0xffffffffffffffff`.
- Оба fixtures содержат один clean-room Configuration record, один Language record и согласованные `root`, `version`, `versions`. Значения и UUID принадлежат этому проекту; BSL, формы и чужие metadata отсутствуют.

Fixture — структурное evidence, а не обещание полной совместимости с любой сборкой платформы. Статус конкретной capability становится `verified` только после соответствующих parser/writer/roundtrip tests.

## Воспроизводимый рецепт

1. Payload — UTF-8 text, сжатый raw DEFLATE (`wbits = -15`, level 9).
2. Element header — 20 нулевых служебных байт, UTF-16LE name и четыре нулевых завершающих байта.
3. TOC сохраняет порядок manifest; data pages имеют не менее 512 байт.
4. Format16 preamble строится как обычный semantic Format15 envelope из тех же специальных entries и расширяет последнюю data page до exact base offset `0x1359`. Недокументированный byte blob не копируется.
5. Полный и составные SHA-256 записаны в manifest и проверяются portable test.

## Независимые первичные источники

Формат сверялся с исходниками, но код из них не копируется и runtime dependency не добавляется:

- [e8tools/v8unpack `V8File.h` at d34bb1e](https://github.com/e8tools/v8unpack/blob/d34bb1e3565572e0de30a4aa4d66d6cd3e3e08e2/src/V8File.h): Format15/16 field widths, sentinels, 55-byte header и base offset `0x1359` (MPL-2.0).
- [e8tools/v8unpack `V8File.cpp` at d34bb1e](https://github.com/e8tools/v8unpack/blob/d34bb1e3565572e0de30a4aa4d66d6cd3e3e08e2/src/V8File.cpp): TOC addressing, read/write allocation и Format16 selection (MPL-2.0).
- [Infactum/onec_dtools `container_reader.py` at 99c0b394](https://github.com/Infactum/onec_dtools/blob/99c0b394f51fbd4225735ab37068c2dae00fdc00/onec_dtools/container_reader.py) и [writer](https://github.com/Infactum/onec_dtools/blob/99c0b394f51fbd4225735ab37068c2dae00fdc00/onec_dtools/container_writer.py): независимый Format15 reader/writer (MIT).

## Optional external corpus

Нормальный CI ничего не скачивает и не запускает. Для локального research test можно создать каталог, положить туда два файла под именами из `external_corpus.artifacts[].local_name` и задать `IBCMD_CF_EXTERNAL_CORPUS`. Тест сверит exact size/SHA, revision header, declared element count и наличие `root/version/versions`. Артефакты содержат сторонний учебный код, поэтому намеренно не vendored; commit, source path и лицензия записаны в manifest.

```powershell
$env:IBCMD_CF_EXTERNAL_CORPUS = 'C:\path\to\private-cf-corpus'
cargo test --locked --test cf_corpus
```
