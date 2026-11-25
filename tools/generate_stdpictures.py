"""Prepare a 1C managed application probe or generate Rust tables from its TSV."""
import argparse
import re
import sys
from pathlib import Path

from compare_stdpictures import NAME, UUID, help_names


def prepare(book, sample, output):
    names = help_names(book)
    if sample:
        names.update(re.findall(rf'"StdPicture\.({NAME})"', sample.read_text(encoding="utf-8-sig")))
    destination = output.resolve().as_posix().replace('"', '""')
    print(f'''Процедура ПриНачалеРаботыСистемы()
    Результат = Новый ЗаписьТекста("{destination}", КодировкаТекста.UTF8);
    Для Каждого Имя Из СтрРазделить("{','.join(sorted(names))}", ",") Цикл
        Попытка
            Значение = ЗначениеВСтрокуВнутр(БиблиотекаКартинок[Имя]);
            Значение = СтрЗаменить(СтрЗаменить(СтрЗаменить(Значение, Символы.ВК, ""), Символы.ПС, ""), Символы.Таб, "");
            Результат.ЗаписатьСтроку(Имя + Символы.Таб + Значение);
        Исключение
            Результат.ЗаписатьСтроку(Имя + Символы.Таб + "ERROR");
        КонецПопытки;
    КонецЦикла;
    Результат.Закрыть();
    ЗавершитьРаботуСистемы(Ложь);
КонецПроцедуры''')


def generate(source, allow_missing):
    seen = set()
    missing = set()
    uuids = {}
    codes = {}
    storage = re.compile(rf'\{{4,1,\{{(?:0,({UUID})|(-[1-9][0-9]*))\}},')
    for line in source.read_text(encoding="utf-8-sig").splitlines():
        name, value = line.split("\t", 1)
        if not re.fullmatch(NAME, name) or name in seen:
            raise ValueError(f"invalid or duplicate name: {name}")
        seen.add(name)
        if value == "ERROR":
            missing.add(name)
            continue
        match = storage.search(value)
        if not match:
            raise ValueError(f"unsupported serialized picture: {name}")
        uuid, code = match.groups()
        table, key = (uuids, uuid.lower()) if uuid else (codes, int(code))
        if key in table:
            raise ValueError(f"duplicate key {key}: {table[key]}, {name}")
        table[key] = name
    if not seen or missing != set(allow_missing):
        raise ValueError(f"unexpected unavailable names: {sorted(missing)}")
    for label, key_type, table in (
        ("STANDARD_PICTURES", "&str", uuids),
        ("STANDARD_PICTURE_CODES", "i32", codes),
    ):
        print(f"pub(crate) const {label}: &[({key_type}, &str)] = &[")
        for key, name in sorted(table.items(), key=lambda row: row[1]):
            literal = f'"{key}"' if isinstance(key, str) else key
            print(f'    ({literal}, "StdPicture.{name}"),')
        print("];\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    probe = commands.add_parser("prepare")
    probe.add_argument("help_book", type=Path)
    probe.add_argument("output_tsv", type=Path)
    probe.add_argument("--sample", type=Path)
    rust = commands.add_parser("generate")
    rust.add_argument("runtime_tsv", type=Path)
    rust.add_argument("--allow-missing", action="append", default=[])
    args = parser.parse_args()
    if args.command == "prepare":
        sys.stdout.reconfigure(encoding="utf-8")
        prepare(args.help_book, args.sample, args.output_tsv)
    else:
        generate(args.runtime_tsv, args.allow_missing)


if __name__ == "__main__":
    main()
