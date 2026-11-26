"""S1 step 1 (issue #391): attributes of every primitive type on catalogs and a document.

Reads the reference native export (never written), writes the edited files - only those - under
<out>/stage/<relative path> and the originals under <out>/before/, ready for
`import_files.ps1 -BaseDir <out>\\stage -Files ...` (a native `import files --partial`).

usage: python edit_cases_s1.py <out dir> [case]      case: t1 (default)
"""
import os
import re
import shutil
import sys
import uuid

REF = os.environ.get(
    "DDL_NATIVE_TREE",
    r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native",
)


def T(name):
    return "<v8:Type>%s</v8:Type>" % name


def string_var(n):
    return [T("xs:string"), "<v8:StringQualifiers>", "\t<v8:Length>%d</v8:Length>" % n,
            "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]


def string_fixed(n):
    return [T("xs:string"), "<v8:StringQualifiers>", "\t<v8:Length>%d</v8:Length>" % n,
            "\t<v8:AllowedLength>Fixed</v8:AllowedLength>", "</v8:StringQualifiers>"]


def number(digits, fraction, sign="Any"):
    return [T("xs:decimal"), "<v8:NumberQualifiers>", "\t<v8:Digits>%d</v8:Digits>" % digits,
            "\t<v8:FractionDigits>%d</v8:FractionDigits>" % fraction,
            "\t<v8:AllowedSign>%s</v8:AllowedSign>" % sign, "</v8:NumberQualifiers>"]


def date(kind):
    return [T("xs:dateTime"), "<v8:DateQualifiers>", "\t<v8:DateFractions>%s</v8:DateFractions>" % kind,
            "</v8:DateQualifiers>"]


BOOLEAN = [T("xs:boolean")]


def attribute_block(name, synonym, type_xml, nl, use=None, tabs=3):
    """An <Attribute> as the 2.20 export writes it; `use` is the catalog's Use (None for a document)."""
    lines = [
        '<Attribute uuid="%s">' % uuid.uuid4(),
        "\t<Properties>",
        "\t\t<Name>%s</Name>" % name,
        "\t\t<Synonym>",
        "\t\t\t<v8:item>",
        "\t\t\t\t<v8:lang>ru</v8:lang>",
        "\t\t\t\t<v8:content>%s</v8:content>" % synonym,
        "\t\t\t</v8:item>",
        "\t\t</Synonym>",
        "\t\t<Comment/>",
        "\t\t<Type>",
    ] + ["\t\t\t" + x for x in type_xml] + [
        "\t\t</Type>",
        "\t\t<PasswordMode>false</PasswordMode>",
        "\t\t<Format/>",
        "\t\t<EditFormat/>",
        "\t\t<ToolTip/>",
        "\t\t<MarkNegatives>false</MarkNegatives>",
        "\t\t<Mask/>",
        "\t\t<MultiLine>false</MultiLine>",
        "\t\t<ExtendedEdit>false</ExtendedEdit>",
        '\t\t<MinValue xsi:nil="true"/>',
        '\t\t<MaxValue xsi:nil="true"/>',
        "\t\t<FillFromFillingValue>false</FillFromFillingValue>",
        '\t\t<FillValue xsi:nil="true"/>',
        "\t\t<FillChecking>DontCheck</FillChecking>",
        "\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>",
        "\t\t<ChoiceParameterLinks/>",
        "\t\t<ChoiceParameters/>",
        "\t\t<QuickChoice>Auto</QuickChoice>",
        "\t\t<CreateOnInput>Auto</CreateOnInput>",
        "\t\t<ChoiceForm/>",
        "\t\t<LinkByType/>",
        "\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>",
    ] + (["\t\t<Use>%s</Use>" % use] if use else []) + [
        "\t\t<Indexing>DontIndex</Indexing>",
        "\t\t<FullTextSearch>Use</FullTextSearch>",
        "\t\t<DataHistory>Use</DataHistory>",
        "\t</Properties>",
        "</Attribute>",
    ]
    return nl.join(("\t" * tabs + x) for x in lines)


class Tree:
    def __init__(self, out, case):
        self.out = out
        self.case = case
        self.stage = os.path.join(out, "stage")
        self.before = os.path.join(out, "before")

    def read(self, rel):
        with open(os.path.join(REF, rel), "rb") as f:
            raw = f.read()
        return raw.decode("utf-8-sig"), raw.startswith(b"\xef\xbb\xbf")

    def write(self, rel, text, bom):
        for base in (self.stage,):
            path = os.path.join(base, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "wb") as f:
                f.write((b"\xef\xbb\xbf" if bom else b"") + text.encode("utf-8"))
        path = os.path.join(self.before, rel)
        if not os.path.exists(path):
            os.makedirs(os.path.dirname(path), exist_ok=True)
            shutil.copyfile(os.path.join(REF, rel), path)


def nl_of(text):
    return "\r\n" if "\r\n" in text else "\n"


def insert_after_last_attribute(text, block, nl):
    needle = nl + "\t\t\t</Attribute>"
    i = text.rfind(needle)
    assert i >= 0, "no attribute to insert after"
    j = i + len(needle)
    return text[:j] + nl + block + text[j:]


def insert_before_attribute(text, index, block, nl):
    """Before the index-th (0-based) object-level <Attribute>."""
    starts = [m.start() for m in re.finditer(r"\t\t\t<Attribute uuid=", text)]
    assert index < len(starts), "the object has %d attributes" % len(starts)
    return text[:starts[index]] + block + nl + text[starts[index]:]


def add_first_attribute(text, block, nl):
    assert text.count("<ChildObjects/>") == 1
    return text.replace("<ChildObjects/>", "<ChildObjects>" + nl + block + nl + "\t\t</ChildObjects>")


# (relative path, [(name, synonym, type, use)], position) ; position: 'end', ('before', i) or 'first'
T1 = [
    ("Catalogs/_ДемоМестаХранения.xml", "end", [
        ("ДемоТ1Дата", "Демо Т1 дата", date("Date"), "ForItem"),
        ("ДемоТ1Число", "Демо Т1 число", number(10, 2), "ForFolderAndItem"),
    ]),
    ("Catalogs/_ДемоГруппыДоступаПартнеров.xml", "end", [
        ("ДемоТ1Булево", "Демо Т1 булево", BOOLEAN, "ForItem"),
    ]),
    ("Catalogs/_ДемоПартнеры.xml", "end", [
        ("ДемоТ1Строка", "Демо Т1 строка", string_var(30), "ForItem"),
        ("ДемоТ1Время", "Демо Т1 время", date("Time"), "ForFolderAndItem"),
        ("ДемоТ1ЧислоПапки", "Демо Т1 число папки", number(5, 0), "ForFolder"),
    ]),
    ("Catalogs/КлючевыеОперации.xml", "end", [
        ("ДемоТ1Булево", "Демо Т1 булево", BOOLEAN, "ForItem"),
        ("ДемоТ1СтрокаПеременная", "Демо Т1 строка переменная", string_var(10), "ForItem"),
        ("ДемоТ1СтрокаФикс", "Демо Т1 строка фикс", string_fixed(20), "ForItem"),
        ("ДемоТ1СтрокаНеогр", "Демо Т1 строка неогр", string_var(0), "ForItem"),
        ("ДемоТ1ЧислоЦелое", "Демо Т1 число целое", number(10, 0), "ForItem"),
        ("ДемоТ1ЧислоДробное", "Демо Т1 число дробное", number(12, 3), "ForItem"),
        ("ДемоТ1ЧислоНеотр", "Демо Т1 число неотр", number(5, 0, "Nonnegative"), "ForItem"),
        ("ДемоТ1Дата", "Демо Т1 дата", date("Date"), "ForItem"),
        ("ДемоТ1ДатаВремя", "Демо Т1 дата время", date("DateTime"), "ForItem"),
    ]),
    ("Catalogs/Удалить_ДемоОбщиеСведения.xml", "first", [
        ("ДемоТ1Первый", "Демо Т1 первый", string_var(15), "ForItem"),
    ]),
    ("Documents/_ДемоЗаказПокупателя.xml", "end", [
        ("ДемоТ1Булево", "Демо Т1 булево", BOOLEAN, None),
        ("ДемоТ1Строка", "Демо Т1 строка", string_var(40), None),
        ("ДемоТ1Число", "Демо Т1 число", number(15, 3), None),
        ("ДемоТ1Дата", "Демо Т1 дата", date("Date"), None),
        ("ДемоТ1ДатаВремя", "Демо Т1 дата время", date("DateTime"), None),
        ("ДемоТ1СтрокаНеогр", "Демо Т1 строка неогр", string_var(0), None),
    ]),
    # a field in the middle of the document's attributes (metadata order decides the column's place)
    ("Documents/_ДемоЗаказПокупателя.xml", ("before", 5), [
        ("ДемоТ1Середина", "Демо Т1 середина", number(3, 0), None),
    ]),
]


def run_t1(out):
    tree = Tree(out, "t1")
    files = {}
    for rel, position, attrs in T1:
        text, bom = files[rel] if rel in files else tree.read(rel)
        nl = nl_of(text)
        is_document = rel.startswith("Documents/")
        for name, synonym, type_xml, use in attrs:
            block = attribute_block(name, synonym, type_xml, nl, use=None if is_document else use)
            if position == "end":
                text = insert_after_last_attribute(text, block, nl)
            elif position == "first":
                text = add_first_attribute(text, block, nl)
            else:
                text = insert_before_attribute(text, position[1], block, nl)
        files[rel] = (text, bom)
    for rel, (text, bom) in files.items():
        tree.write(rel, text, bom)
        print("edited", rel)
    with open(os.path.join(out, "files.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(sorted(files)) + "\n")


if __name__ == "__main__":
    out = sys.argv[1]
    case = sys.argv[2] if len(sys.argv) > 2 else "t1"
    if case == "t1":
        run_t1(out)
    else:
        raise SystemExit("unknown case " + case)
