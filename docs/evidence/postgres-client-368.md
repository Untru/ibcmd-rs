# Проверка встроенного клиента PostgreSQL (#368)

Windows, PostgreSQL 17.2-4.1C, штатный `ibcmd` 8.5.1.1529. Исходные файлы конфигурации взяты из `tests/fixtures/file-infobase/tiny-source`. Для воспроизведения нужен отдельный кластер с UTF-8, локалью `Russian_Russia.1251`, входом SCRAM-SHA-256 и свободным портом. Локаль `C` была отвергнута штатным `ibcmd` при установке порядка сортировки.

## Подготовка эталона

В PowerShell задайте `$Native` (путь к штатному ibcmd), `$Lab` (пустой каталог), `$Port` (порт отдельного кластера) и `IBCMD_RS_PG_PASSWORD` (пароль временной роли postgres). Из корня репозитория выполните команды ниже. Штатная платформа создаёт базу `ib368`, импортирует конфигурацию, применяет её и формирует эталон до первой записи встроенным клиентом.

```powershell
$Source = (Resolve-Path tests/fixtures/file-infobase/tiny-source).Path
$Common = @('--dbms=PostgreSQL', "--db-server=127.0.0.1 port=$Port",
    '--db-name=ib368', '--db-user=postgres',
    "--db-pwd=$env:IBCMD_RS_PG_PASSWORD", "--data=$Lab/data")
& $Native infobase create @Common --create-database
& $Native infobase config import @Common $Source
& $Native infobase config apply @Common --force
& $Native infobase config export @Common "$Lab/oracle"
```

## Проверки клиента

Включите `ssl=on` для кластера, установите серверный сертификат с IP-адресом 127.0.0.1 в SAN и укажите доверенный центр в `SSL_CERT_FILE`. Оставьте для этого адреса только правило `hostssl all all 127.0.0.1/32 scram-sha-256` и перечитайте конфигурацию. Для наблюдения за операциями включите `log_statement=all` на одноразовом кластере.

```powershell
$env:SSL_CERT_FILE = "$Lab/ca.pem"
$env:IBCMD_RS_PG_SERVER = "127.0.0.1 port=$Port; sslmode=verify-full"
$env:IBCMD_RS_PG_DATABASE = 'ib368'
$env:IBCMD_RS_PG_USER = 'postgres'
cargo test --locked -p ibcmd-postgres --lib
cargo test --locked -p ibcmd-postgres --test live -- --ignored
cargo test --locked -p ibcmd-sql --test postgres_live -- --ignored
$env:IBCMD_DB_PSW = $env:IBCMD_RS_PG_PASSWORD
$Db = @('--dbms=PostgreSQL', "--db-server=$env:IBCMD_RS_PG_SERVER",
    '--db-name=ib368', '--db-user=postgres')
cargo build --locked --bin ibcmd-rs
$Cli = (Resolve-Path target/debug/ibcmd-rs.exe).Path
& $Cli infobase config export @Db "$Lab/read"
& $Cli infobase config import @Db $Source
& $Cli infobase config export @Db --platform=8.5.1 "$Lab/reread"
```

При заданном `CARGO_TARGET_DIR` подставьте каталог этой сборки в путь к CLI. Сопоставьте деревья по относительным путям и байтам. Для проверки отказов повторите первый экспорт с новым каталогом вывода без `sslmode=verify-full`, затем с проверкой TLS, удалив из доверенных корень временного центра.

Команды сверки для PowerShell 7 сравнивают относительные пути и SHA-256. При полном совпадении вывод пуст. В первой сверке участвует служебный XML версий.

```powershell
function Get-TreeDigest($Root, [switch]$SkipVersions) {
    $base = (Resolve-Path -LiteralPath $Root).Path
    Get-ChildItem -LiteralPath $base -Recurse -File | ForEach-Object {
        $name = [IO.Path]::GetRelativePath($base, $_.FullName)
        if (!$SkipVersions -or $name -cne 'ConfigDumpInfo.xml') {
            $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            "$name $hash"
        }
    }
}
Compare-Object -CaseSensitive (Get-TreeDigest "$Lab/oracle") (Get-TreeDigest "$Lab/read")
Compare-Object -CaseSensitive (Get-TreeDigest tests/fixtures/file-infobase/tiny-xml -SkipVersions) (Get-TreeDigest "$Lab/reread" -SkipVersions)
```

Потоковый unit-тест задерживает завершение запроса на стороне сервера до того, как обработчик клиента получит первую строку. Live-тест отдельно проверяет TLS текущего сеанса в `pg_stat_ssl`, порядок всех 10 000 значений `generate_series`, запись NULL, текста и двоичных байтов. После отмены COPY, нарушения UNIQUE и отказа сервера до начала COPY он сверяет содержимое временной таблицы на том же соединении.

## Результаты 24.11.2026

| Проверка | Наблюдение |
|---|---|
| Чтение опубликованной конфигурации | Перед экспортом `EXISTS (SELECT 1 FROM config)` и `NOT EXISTS (SELECT 1 FROM configsave)` истинны. Оба CLI завершились с кодом 0. Совпали пути и байты 5 из 5 файлов, включая `ConfigDumpInfo.xml`. |
| Проверка транспорта | Вход по SCRAM на `hostssl` прошёл. В том же сеансе live-теста `pg_stat_ssl.ssl = true`. Запуск CLI по обычному TCP завершился ошибкой правила входа. При исключении временного центра из доверенных клиент вернул `UnknownIssuer`. |
| Потоковое чтение | Тест с задержкой на стороне сервера прошёл. Live-тест получил по порядку все 10 000 значений без накопления результата запроса. Экспорт после чтения собирает конфигурацию в памяти. |
| Запись через CLI | В журнале сервера одна команда `COPY pg_temp.ibcmd_stage (name, attributes, size, data, part) FROM STDIN BINARY` для импорта. После фиксации транзакции `configsave` заполнена. Повторный экспорт дал те же пути и байты для 4 из 4 файлов конфигурации, что и `tests/fixtures/file-infobase/tiny-xml`. Служебный `ConfigDumpInfo.xml` исключён из этой второй сверки, поскольку при импорте создаются новые версии. |
| Отказ при COPY | Live-тест получил ошибку производителя, SQLSTATE 23505 при дубликате и 42P01 при неизвестной таблице. Сохранились две успешно записанные строки. Следующий запрос на том же соединении прошёл. |

Общий интерфейс `SqlClient` подключает PostgreSQL через `SqlExec::postgres`. Live-тест читает имена и двоичные поля из `Config` с параметром номера части, проверяет JSON, связывание каждого типа `SqlParam`, SQLSTATE при нарушении уникальности и запись через общий метод `write_rows`. Для проверки записи тест создаёт и удаляет отдельную таблицу. CLI сохраняет транзакционные сессии для чтения снимка и записи ConfigSave. Импорт и загрузка CF используют тот же бинарный кодировщик, что и `write_rows`.
