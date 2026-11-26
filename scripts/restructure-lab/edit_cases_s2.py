"""S1 wave 1 (issue #391: S1-B delete an attribute, S1-C widen a string, S1-D the index flag) on the pristine БСП.

Reads the reference native export (never written), writes the edited files - only those - under
<out>/stage/<relative path> and the originals under <out>/before/, ready for
`import_files.ps1 -BaseDir <out>\\stage -Files ...` (a native `import files --partial`).

usage: python edit_cases_s2.py <out dir> <case>
  b1   S1-B: delete attributes - the middle, the last, the first, an indexed one, the only one of an object (also indexed),
       several in one object, a nullable one of a hierarchical catalog, one of a document - and delete + add in one stage
  b2   S1-B: an attribute with the additional-order index (catalog and document: the document's ByDocDate names it)
  c1   S1-C: widen variable strings (catalog, hierarchical catalog, document, indexed attributes, up to 1024)
  d0   S1-D: ONE flag alone (the trace of what the platform does)
  d1   S1-D: switch the index on and off (string and number, hierarchical / flat catalog, document), additional order
"""
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from edit_cases_s1 import (REF, attribute_block, nl_of, number, string_var)  # noqa: E402


class Tree:
    """Reads a file from the reference tree; writes the edited ones."""

    def __init__(self, out):
        self.out = out
        self.stage = os.path.join(out, "stage")
        self.before = os.path.join(out, "before")
        self.files = {}

    def get(self, rel):
        if rel not in self.files:
            with open(os.path.join(REF, rel), "rb") as f:
                raw = f.read()
            self.files[rel] = [raw.decode("utf-8-sig"), raw.startswith(b"\xef\xbb\xbf")]
        return self.files[rel][0]

    def put(self, rel, text):
        self.get(rel)
        self.files[rel][0] = text

    def save(self):
        for rel, (text, bom) in self.files.items():
            path = os.path.join(self.stage, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "wb") as f:
                f.write((b"\xef\xbb\xbf" if bom else b"") + text.encode("utf-8"))
            before = os.path.join(self.before, rel)
            if not os.path.exists(before):
                os.makedirs(os.path.dirname(before), exist_ok=True)
                shutil.copyfile(os.path.join(REF, rel), before)
            print("edited", rel)
        with open(os.path.join(self.out, "files.txt"), "w", encoding="utf-8") as f:
            f.write("\n".join(sorted(self.files)) + "\n")


def append_attribute(tree, rel, name, synonym, type_xml, use=None, indexing="DontIndex"):
    text = tree.get(rel)
    nl = nl_of(text)
    block = attribute_block(name, synonym, type_xml, nl, use=use)
    assert "<Indexing>DontIndex</Indexing>" in block
    block = block.replace("<Indexing>DontIndex</Indexing>", "<Indexing>%s</Indexing>" % indexing)
    needle = nl + "\t\t\t</Attribute>"
    i = text.rfind(needle)
    if i < 0 and text.count("<ChildObjects/>") == 1:
        text = text.replace("<ChildObjects/>", "<ChildObjects>" + nl + block + nl + "\t\t</ChildObjects>")
    elif i < 0:
        # no attribute left: the attributes come first among the children
        opening = nl + "\t\t<ChildObjects>"
        assert text.count(opening) == 1
        text = text.replace(opening, opening + nl + block)
    else:
        j = i + len(needle)
        text = text[:j] + nl + block + text[j:]
    tree.put(rel, text)


def attribute_span(text, name):
    """(start, end) of the object-level <Attribute> named `name`, the end after its closing tag."""
    nl = nl_of(text)
    marker = "<Name>%s</Name>" % name
    for m in re.finditer(re.escape(marker), text):
        position = m.start()
        start = text.rfind(nl + "\t\t\t<Attribute uuid=", 0, position)
        if start < 0:
            continue
        between = text[start + len(nl):position]
        if between.count("<Attribute") != 1:
            continue
        end = text.find(nl + "\t\t\t</Attribute>", position)
        assert end > 0
        return start, end + len(nl + "\t\t\t</Attribute>")
    raise SystemExit("no object-level attribute named %s" % name)


def remove_attribute(tree, rel, name):
    text = tree.get(rel)
    start, end = attribute_span(text, name)
    text = text[:start] + text[end:]
    nl = nl_of(text)
    empty = "<ChildObjects>" + nl + "\t\t</ChildObjects>"
    if empty in text:
        text = text.replace(empty, "<ChildObjects/>")
    tree.put(rel, text)


def edit_attribute(tree, rel, name, old, new):
    text = tree.get(rel)
    start, end = attribute_span(text, name)
    block = text[start:end]
    assert block.count(old) == 1, "%s: %r appears %d times" % (name, old, block.count(old))
    tree.put(rel, text[:start] + block.replace(old, new) + text[end:])


def flip(tree, rel, name, old, new):
    edit_attribute(tree, rel, name, "<Indexing>%s</Indexing>" % old, "<Indexing>%s</Indexing>" % new)


def widen(tree, rel, name, old, new):
    edit_attribute(tree, rel, name, "<v8:Length>%d</v8:Length>" % old, "<v8:Length>%d</v8:Length>" % new)


KEYOPS = "Catalogs/КлючевыеОперации.xml"  # flat, 7 attributes, four of them indexed
SERVICES = "Catalogs/ПоставщикиУслугСервиса.xml"  # one attribute, indexed
BANKS = "Catalogs/КлассификаторБанков.xml"  # hierarchical, 14 attributes, nullable columns
CALL = "Documents/ТелефонныйЗвонок.xml"  # 11 attributes, the first indexed, two tabular sections
VAT = "Catalogs/_ДемоСтавкиНДС.xml"  # one attribute
JOBS = "Catalogs/ШаблоныЗаданийОчереди.xml"  # 7 attributes
PROJECTS = "Catalogs/_ДемоПроекты.xml"  # an attribute with the additional-order index
PAYROLL = "Documents/_ДемоНачислениеЗарплаты.xml"  # an attribute with the additional-order index
VOLUMES = "Catalogs/ТомаХраненияФайлов.xml"
ORDER = "Documents/_ДемоЗаказПокупателя.xml"


def run_b1(tree):
    # several in the flat catalog: an indexed one in the middle, a plain one, the last (indexed)
    remove_attribute(tree, KEYOPS, "ИмяХеш")
    remove_attribute(tree, KEYOPS, "ЦелевоеВремя")
    remove_attribute(tree, KEYOPS, "Длительная")
    # the only attribute of an object, indexed
    remove_attribute(tree, SERVICES, "Идентификатор")
    # a hierarchical catalog: the first (indexed, nullable), a string in the middle, the last
    remove_attribute(tree, BANKS, "КоррСчет")
    remove_attribute(tree, BANKS, "МеждународноеНаименование")
    remove_attribute(tree, BANKS, "КодСтраны")
    # a document: the first (indexed), the last (unlimited string)
    remove_attribute(tree, CALL, "АбонентКакСвязаться")
    remove_attribute(tree, CALL, "Комментарий")
    # delete + add in one stage: the only attribute replaced; a middle one replaced
    remove_attribute(tree, VAT, "Ставка")
    append_attribute(tree, VAT, "СтавкаНовая", "Ставка новая", number(5, 2), "ForItem")
    remove_attribute(tree, JOBS, "Ключ")
    append_attribute(tree, JOBS, "КлючНовый", "Ключ новый", string_var(64), "ForItem")


def run_b2(tree):
    remove_attribute(tree, PROJECTS, "РеквизитДопУпорядочивания")
    remove_attribute(tree, PAYROLL, "ПериодРегистрации")


def run_c1(tree):
    widen(tree, JOBS, "ИмяМетода", 255, 1024)  # up to the limit of a limited string
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)  # indexed
    widen(tree, BANKS, "Город", 50, 200)  # hierarchical catalog
    widen(tree, BANKS, "ИНН", 12, 20)  # hierarchical, indexed
    widen(tree, CALL, "АбонентКакСвязаться", 100, 300)  # document, indexed
    widen(tree, ORDER, "ГородДоставки", 50, 51)  # document, by one


def run_d0(tree):
    flip(tree, KEYOPS, "ЦелевоеВремя", "DontIndex", "Index")


def run_d1(tree):
    # on: a number and a string, in the flat catalog, the hierarchical catalog, the document; additional order on
    flip(tree, KEYOPS, "ЦелевоеВремя", "DontIndex", "Index")
    flip(tree, BANKS, "Город", "DontIndex", "Index")
    flip(tree, BANKS, "Телефоны", "DontIndex", "IndexWithAdditionalOrder")
    flip(tree, VOLUMES, "МаксимальныйРазмер", "DontIndex", "Index")
    flip(tree, ORDER, "РегионДоставки", "DontIndex", "Index")
    flip(tree, ORDER, "ДатаДоставки", "DontIndex", "IndexWithAdditionalOrder")
    # off: indexed ones of the БСП, also with additional order
    flip(tree, KEYOPS, "Приоритет", "Index", "DontIndex")
    flip(tree, KEYOPS, "ВыполненаСОшибкой", "Index", "DontIndex")
    flip(tree, VOLUMES, "ПорядокЗаполнения", "Index", "DontIndex")
    flip(tree, ORDER, "СтатусЗаказа", "Index", "DontIndex")
    flip(tree, PROJECTS, "РеквизитДопУпорядочивания", "IndexWithAdditionalOrder", "DontIndex")
    flip(tree, PAYROLL, "ПериодРегистрации", "IndexWithAdditionalOrder", "DontIndex")


CASES = {"b1": run_b1, "b2": run_b2, "c1": run_c1, "d0": run_d0, "d1": run_d1}

if __name__ == "__main__":
    out = sys.argv[1]
    case = sys.argv[2]
    if case not in CASES:
        raise SystemExit("unknown case %s; known: %s" % (case, ", ".join(CASES)))
    tree = Tree(out)
    CASES[case](tree)
    tree.save()
