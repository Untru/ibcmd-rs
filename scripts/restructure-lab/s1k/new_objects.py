"""S1-F cases for S1-K (#407, #402): a new catalog and a new document with attributes, added to the БСП 8.3.27 tree.

The shapes are the trace track's N1 (a flat catalog: string, number, date, boolean, a reference, an indexed string, one attribute
with an additional order) and N4 (a document with a periodic numeric number, an indexed attribute and one with an additional order),
`scripts/apply-trace/lab/s1g-caches/make_cases_n.py` -- rewritten here as functions, because that file writes its cases when it is
imported. Limits of S1-F (docs/apply/new-object.md 3.3): no forms, templates, commands or predefined items on the new object, one
new object per kind per stage, nothing of an object an extension adopts.

The catalog is made from `skeleton_catalog.xml` (a catalog the platform created: flat, code 9, description 25, no attributes, no
forms; the ddl track's case c), the document from `Documents/_ДемоОприходованиеТоваров.xml` of the tree (its forms, register records and
catalog references taken out). Every uuid of a copy is new; the object is listed in `Configuration.xml` at its alphabetical place.
"""
import os
import re
import sys
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import edit_cases_s1 as s1  # noqa: E402

LF, CR = "\n", "\r"
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
SKELETON = os.path.join(HERE, "skeleton_catalog.xml")


def read_ref(rel):
    with open(os.path.join(s1.REF, rel), "rb") as f:
        raw = f.read()
    return raw.decode("utf-8-sig"), raw.startswith(b"\xef\xbb\xbf")


def lf(text):
    return text.replace(CR + LF, LF)


def with_line_ends(text, like):
    """`text` (LF) with the line ends of `like` (the file it takes its place beside)."""
    return text.replace(LF, CR + LF) if CR + LF in like else text


def renew(text):
    """Every uuid of the text becomes a new one (the same old uuid the same new one)."""
    mapping = {}
    return UUID.sub(lambda m: mapping.setdefault(m.group(0), str(uuid.uuid4())), text)


TYPES = {
    "string": lambda n: ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>%d</v8:Length>" % n,
                         "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"],
    "number": lambda d, f: ["<v8:Type>xs:decimal</v8:Type>", "<v8:NumberQualifiers>", "\t<v8:Digits>%d</v8:Digits>" % d,
                            "\t<v8:FractionDigits>%d</v8:FractionDigits>" % f, "\t<v8:AllowedSign>Any</v8:AllowedSign>",
                            "</v8:NumberQualifiers>"],
    "date": lambda: ["<v8:Type>xs:dateTime</v8:Type>", "<v8:DateQualifiers>", "\t<v8:DateFractions>Date</v8:DateFractions>",
                     "</v8:DateQualifiers>"],
    "boolean": lambda: ["<v8:Type>xs:boolean</v8:Type>"],
    "ref": lambda name: ["<v8:Type>cfg:%s</v8:Type>" % name],
}


def attribute(name, synonym, type_xml, use=None, indexing="DontIndex"):
    """An object-level <Attribute> block (three tabs), as the 2.20 export writes it; `use` is a catalog attribute's Use."""
    block = s1.attribute_block(name, synonym, type_xml, LF, use=use)
    if indexing != "DontIndex":
        assert "<Indexing>DontIndex</Indexing>" in block
        block = block.replace("<Indexing>DontIndex</Indexing>", "<Indexing>%s</Indexing>" % indexing)
    return block


def catalog(name, synonym, attributes):
    """(text, bom) of a new flat catalog with the attributes, from the skeleton."""
    with open(SKELETON, "rb") as f:
        raw = f.read()
    text = lf(raw.decode("utf-8-sig"))
    old = "ДемоНовыйСправочник"
    text = renew(text).replace(old, name)
    assert "<v8:content>Демо: Новый справочник</v8:content>" in text
    text = text.replace("<v8:content>Демо: Новый справочник</v8:content>", "<v8:content>%s</v8:content>" % synonym, 1)
    assert "\t\t<ChildObjects/>" in text
    text = text.replace("\t\t<ChildObjects/>", "\t\t<ChildObjects>" + LF + LF.join(attributes) + LF + "\t\t</ChildObjects>", 1)
    return text, raw.startswith(b"\xef\xbb\xbf")


def document(name, synonym, extra_attributes, number=None):
    """(text, bom) of a new document from `_ДемоОприходованиеТоваров`, with its forms, register records and catalog attributes out."""
    raw_text, bom = read_ref("Documents/_ДемоОприходованиеТоваров.xml")
    skeleton = "_ДемоОприходованиеТоваров"
    text = renew(lf(raw_text)).replace(skeleton, name)
    text = text.replace("Демо: Оприходование товаров", synonym).replace("Демо: Оприходования товаров", synonym + " (список)")
    text = re.sub(r"<DefaultObjectForm>[^<]*</DefaultObjectForm>", "<DefaultObjectForm/>", text)
    text = re.sub(r"<DefaultListForm>[^<]*</DefaultListForm>", "<DefaultListForm/>", text)
    text = re.sub(r"\t\t\t<Form>[^<]*</Form>\n", "", text)
    text = re.sub(r"<RegisterRecords>.*?</RegisterRecords>", "<RegisterRecords/>", text, flags=re.S)
    for attr in ("МестоХранения", "Организация", "Ответственный", "Номенклатура"):
        pattern = re.compile(r'[ \t]*<Attribute uuid="[^"]+">\s*<Properties>\s*<Name>' + attr + r"</Name>.*?</Attribute>\n", re.S)
        text, n = pattern.subn("", text, count=1)
        assert n == 1, attr
    for key, value in (number or {}).items():
        text = re.sub(r"<%s>[^<]*</%s>" % (key, key), "<%s>%s</%s>" % (key, value, key), text, count=1)
    assert "cfg:CatalogRef" not in text, "a catalog reference is left in the document"
    marker = "\t\t\t<TabularSection uuid="
    at = text.index(marker)
    text = text[:at] + LF.join(extra_attributes) + LF + text[at:]
    return text, bom


def list_in_configuration(text, kind, name):
    """`Configuration.xml` text with `<kind>name</kind>` listed at its alphabetical place among the objects of the kind."""
    existing = re.findall(r"<%s>([^<]+)</%s>" % (kind, kind), text)
    later = [n for n in existing if n > name]
    line = "\t\t\t<%s>%s</%s>" % (kind, name, kind)
    if later:
        key = "\t\t\t<%s>%s</%s>" % (kind, later[0], kind)
        assert key in text, key
        return text.replace(key, line + LF + key, 1)
    key = "\t\t\t<%s>%s</%s>" % (kind, existing[-1], kind)
    return text.replace(key, key + LF + line, 1)


class NewObjectTree:
    """The tree of a case that adds files: the edited `Configuration.xml` and the new object files.

    Writes `<out>/stage/<rel>` for every file, `<out>/before/<rel>` for the ones that exist in the reference tree, `files.txt`
    (all of them) and `new.txt` (the ones that are new: `overlay.py` puts them in the working tree and takes them out again)."""

    def __init__(self, out):
        self.out = out
        self.edited = {}
        self.new = {}

    def configuration(self):
        text, bom = read_ref("Configuration.xml")
        return text, bom

    def add(self, rel, text, bom, like):
        self.new[rel] = (with_line_ends(lf(text), like), bom)

    def edit(self, rel, text, bom):
        self.edited[rel] = (text, bom)

    def save(self):
        import shutil
        names = []
        for rel, (text, bom) in sorted({**self.edited, **self.new}.items()):
            stage = os.path.join(self.out, "stage", rel)
            os.makedirs(os.path.dirname(stage), exist_ok=True)
            with open(stage, "wb") as f:
                f.write((b"\xef\xbb\xbf" if bom else b"") + text.encode("utf-8"))
            if rel not in self.new:
                before = os.path.join(self.out, "before", rel)
                os.makedirs(os.path.dirname(before), exist_ok=True)
                shutil.copyfile(os.path.join(s1.REF, rel), before)
            names.append(rel)
            print("edited" if rel not in self.new else "added", rel)
        with open(os.path.join(self.out, "files.txt"), "w", encoding="utf-8") as f:
            f.write("\n".join(names) + "\n")
        with open(os.path.join(self.out, "new.txt"), "w", encoding="utf-8") as f:
            f.write("\n".join(sorted(self.new)) + "\n")


def run_f1(out):
    """N1: a new flat catalog with attributes of every primitive type, a reference, an indexed one and one with an additional order."""
    name = "ДемоКатФ1"
    attributes = [
        attribute("Строка50", "Строка 50", TYPES["string"](50), use="ForItem"),
        attribute("Число", "Число", TYPES["number"](10, 2), use="ForItem"),
        attribute("Дата", "Дата", TYPES["date"](), use="ForItem"),
        attribute("Флаг", "Флаг", TYPES["boolean"](), use="ForItem"),
        attribute("Пользователь", "Пользователь", TYPES["ref"]("CatalogRef.Пользователи"), use="ForItem"),
        attribute("ИндексСтрока", "Индекс строка", TYPES["string"](30), use="ForItem", indexing="Index"),
        attribute("ПорядокСтрока", "Порядок строка", TYPES["string"](20), use="ForItem", indexing="IndexWithAdditionalOrder"),
    ]
    tree = NewObjectTree(out)
    text, bom = catalog(name, "Демо: каталог Ф1", attributes)
    root, root_bom = tree.configuration()
    tree.add("Catalogs/%s.xml" % name, text, bom, root)
    tree.edit("Configuration.xml", list_in_configuration(root, "Catalog", name), root_bom)
    tree.save()


def run_f2(out):
    """N4: a new document with a periodic numeric number, an indexed attribute and one with an additional order."""
    name = "ДемоДокФ2"
    extra = [
        attribute("ИндексЧисло", "Индекс число", TYPES["number"](9, 0), indexing="Index"),
        attribute("ПорядокСтрока", "Порядок строка", TYPES["string"](20), indexing="IndexWithAdditionalOrder"),
    ]
    tree = NewObjectTree(out)
    text, bom = document(name, "Демо: документ Ф2", extra,
                         number={"NumberType": "Number", "NumberLength": "12", "NumberPeriodicity": "Year"})
    root, root_bom = tree.configuration()
    tree.add("Documents/%s.xml" % name, text, bom, root)
    tree.edit("Configuration.xml", list_in_configuration(root, "Document", name), root_bom)
    tree.save()
