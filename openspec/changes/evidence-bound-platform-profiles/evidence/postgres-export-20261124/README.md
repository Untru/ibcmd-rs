# Выгрузка поставочной БСП из PostgreSQL (#369)

## Вход и воспроизведение

Для каждой сборки платформы используется отдельная одноразовая база PostgreSQL
17.2-4.1C x64 с кодировкой UTF-8 и сортировкой Russian_Russia.1251.
Исходный CF БСП 3.1.11.309 взят из официальной поставки. Размер 67 167 823
байта, SHA-256 `4ded13b6b6b5c74f11c544e6be442cbea6f49eb8dede3424841b4bd6af4b4448`.

Проверяемый бинарник ibcmd-rs 0.4.1 собран с `--release --locked` из
`54b796f64da5daed3714acdf7810581fe0e58778`, SHA-256
`f62f0728337ca655a5fab5863ed55813eb7dba2044ee8106e2ddc64341aee3ab`.
Команды ниже используют один сервер, одну базу и один CF внутри каждой пары.
Параметр `<профиль>` задаёт версию XML целевой платформы.

```text
initdb -D <кластер> -U <пользователь> --encoding=UTF8 --locale=Russian_Russia.1251
ibcmd infobase create --create-database --dbms=PostgreSQL --db-server=<сервер> --db-user=<пользователь> --db-name=<база> --data=<рабочий-каталог>
ibcmd infobase config load --dbms=PostgreSQL --db-server=<сервер> --db-user=<пользователь> --db-name=<база> --data=<рабочий-каталог> <БСП.cf>
ibcmd infobase config export --dbms=PostgreSQL --db-server=<сервер> --db-user=<пользователь> --db-name=<база> --data=<рабочий-каталог> <штатный-каталог>
ibcmd-rs infobase config export --dbms=PostgreSQL --db-server=<сервер> --db-user=<пользователь> --db-name=<база> --platform=<профиль> <каталог-кандидата>
python compare.py <штатный-каталог> <каталог-кандидата>
```

Скрипт сравнения требует Python 3.12+. Он сопоставляет точные относительные
имена, сравнивает содержимое блоками по 1 МиБ и считает односторонние файлы.
В SHA-256 дерева входят для каждого файла в порядке сортировки относительного
имени его путь в UTF-8 с разделителем `/`, байт NUL и 32 байта SHA-256 файла.
`ConfigDumpInfo.xml` участвует наравне с остальными файлами.
