"""S1 combinations (issue #391, before 0.4.0): stages that combine the built operations -- A add an attribute, B delete one, C widen a
variable string, D switch the index, E add a tabular section or an attribute of one -- across several objects that no extension adopts,
on the pristine БСП.

Reads the reference native export (never written), writes the edited files - only those - under
<out>/stage/<relative path> and the originals under <out>/before/, ready for a native `import files --partial` (stage_case.ps1).

usage: python edit_cases_s4.py <out dir> <case>
  m1  B + C on the same object (a hierarchical catalog, a flat one, a flat one with indexed attributes)
  m2  C + D on the same attribute (widen and switch the index, on and off, with the additional order), catalog, hierarchical catalog, document
  m3  E + C (+ A) on one object: a new section and a widened own attribute, a new own attribute; a document with a new section, a new attribute of an old
      section, a widened attribute and a new own attribute
  m4  B on some objects and E on others
  m5  A, B, C, D and E in one stage on six objects
  m6  one large stage: 17 objects, every operation
  m7  A + D on the same new attribute: attributes that are added with the index flag (Index, IndexWithAdditionalOrder), in a flat catalog, a
      hierarchical catalog (a nullable one too) and a document
  r1  REFUSED: B and a new section on the same object
  r2  REFUSED: B and a new attribute of an old section on the same object
  r3  REFUSED: valid operations, and a change of an object an extension adopts
  r4  REFUSED: valid operations, and a string that gets shorter
  r5  REFUSED: valid operations, and an attribute that changes its type
  r6  REFUSED: valid operations, and attributes of a subordinate catalog (its owner field is not covered)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from edit_cases_s1 import BOOLEAN, date, nl_of, number, string_var  # noqa: E402
from edit_cases_s2 import (  # noqa: E402
    BANKS, CALL, JOBS, KEYOPS, ORDER, PAYROLL, PROJECTS, SERVICES, VOLUMES, Tree, append_attribute, attribute_span, flip,
    remove_attribute, widen,
)
from edit_cases_s3 import add_section, add_section_attribute, n, s, section_attribute  # noqa: E402

CONTACTS = "Catalogs/_ДемоКонтактныеЛицаПартнеров.xml"  # subordinate, one section
UNITS = "Catalogs/_ДемоПодразделения.xml"  # hierarchical, one section
INTAKE = "Documents/_ДемоОприходованиеТоваров.xml"  # one section
INVOICE = "Documents/_ДемоСчетФактураПолученный.xml"
LEAVE = "Documents/_ДемоОтпускаСотрудников.xml"  # one section
BANK_ACCOUNTS = "Catalogs/_ДемоБанковскиеСчета.xml"  # 26 attributes
COMPONENTS = "Catalogs/ВнешниеКомпоненты.xml"
NOTICES = "Catalogs/ОповещенияПользователей.xml"
CASH = "Documents/_ДемоРасходныйКассовыйОрдер.xml"
PAYMENT = "Documents/_ДемоСписаниеБезналичныхДенежныхСредств.xml"
ADOPTED = "Catalogs/_ДемоПартнеры.xml"  # an extension adopts it: S1-I refuses a stage that changes it
QUEUE = "Catalogs/ОчередьЗаданий.xml"


def section_of(tree, rel, name, attributes):
    add_section(tree, rel, name, name, attributes)


def own(tree, rel, name="ДемоРеквизит", size=20, use=None, indexing="DontIndex"):
    append_attribute(tree, rel, name, "Демо реквизит", string_var(size), use, indexing)


def retype(tree, rel, name, type_xml):
    """The type of an object-level attribute replaced by `type_xml` (lines as string_var / number give them)."""
    text = tree.get(rel)
    nl = nl_of(text)
    start, end = attribute_span(text, name)
    block = text[start:end]
    i = block.index("<Type>")
    j = block.index("</Type>") + len("</Type>")
    indent = block[block.rfind(nl, 0, i) + len(nl):i]
    new = "<Type>" + nl + nl.join(indent + "\t" + line for line in type_xml) + nl + indent + "</Type>"
    tree.put(rel, text[:start] + block[:i] + new + block[j:] + text[end:])


def run_m1(tree):
    remove_attribute(tree, BANKS, "КоррСчет")
    remove_attribute(tree, BANKS, "МеждународноеНаименование")
    widen(tree, BANKS, "Город", 50, 200)
    widen(tree, BANKS, "Адрес", 500, 1000)
    widen(tree, BANKS, "ИНН", 12, 20)
    remove_attribute(tree, JOBS, "Ключ")
    widen(tree, JOBS, "ИмяМетода", 255, 1024)
    widen(tree, JOBS, "Имя", 255, 300)
    remove_attribute(tree, KEYOPS, "Длительная")
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)


def run_m2(tree):
    # widen and switch the index of the same attribute
    widen(tree, BANKS, "Город", 50, 200)
    flip(tree, BANKS, "Город", "DontIndex", "Index")
    widen(tree, BANKS, "ИНН", 12, 20)
    flip(tree, BANKS, "ИНН", "Index", "DontIndex")
    widen(tree, BANKS, "Телефоны", 250, 400)
    flip(tree, BANKS, "Телефоны", "DontIndex", "IndexWithAdditionalOrder")
    widen(tree, ORDER, "РегионДоставки", 50, 60)
    flip(tree, ORDER, "РегионДоставки", "DontIndex", "Index")
    widen(tree, ORDER, "ГородДоставки", 50, 70)
    flip(tree, ORDER, "ГородДоставки", "DontIndex", "IndexWithAdditionalOrder")
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)
    flip(tree, KEYOPS, "ИмяХеш", "Index", "DontIndex")


def run_m3(tree):
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20), n("ДемоЧисло", 10, 2, "Index")])
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)
    section_of(tree, ORDER, "ДемоТЧ2", [("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex")])
    add_section_attribute(tree, ORDER, "СчетаНаОплату", section_attribute(*s("ДемоСтрока", 20)))
    widen(tree, ORDER, "ГородДоставки", 50, 60)
    own(tree, ORDER)
    section_of(tree, CALL, "ДемоТЧ", [s("ДемоСтрока", 10)])
    own(tree, CALL)


def run_m4(tree):
    remove_attribute(tree, BANKS, "КоррСчет")
    remove_attribute(tree, BANKS, "КодСтраны")
    remove_attribute(tree, JOBS, "Ключ")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20), n("ДемоЧисло", 10, 2)])
    section_of(tree, INVOICE, "ДемоТЧ", [s("ДемоСтрока", 15)])
    add_section_attribute(tree, INTAKE, "Товары", section_attribute(*n("ДемоЧисло", 15, 3)), after="")
    add_section_attribute(tree, INTAKE, "Товары", section_attribute(*s("ДемоСтрока", 10, "Index")))


def five_objects(tree):
    remove_attribute(tree, BANKS, "КоррСчет")
    widen(tree, BANKS, "Город", 50, 200)
    flip(tree, BANKS, "Телефоны", "DontIndex", "IndexWithAdditionalOrder")
    remove_attribute(tree, JOBS, "Ключ")
    own(tree, JOBS, "КлючНовый", 64, "ForItem")
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)
    flip(tree, KEYOPS, "ЦелевоеВремя", "DontIndex", "Index")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20), n("ДемоЧисло", 10, 2, "Index")])
    own(tree, ORDER)
    flip(tree, ORDER, "РегионДоставки", "DontIndex", "Index")
    section_of(tree, ORDER, "ДемоТЧ2", [("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex")])
    add_section_attribute(tree, ORDER, "СчетаНаОплату", section_attribute(*s("ДемоСтрока", 20)))
    flip(tree, VOLUMES, "МаксимальныйРазмер", "DontIndex", "Index")


def run_m5(tree):
    five_objects(tree)
    own(tree, UNITS, "ДемоРеквизит", 20, "ForItem")
    add_section_attribute(tree, UNITS, "Сотрудники", section_attribute(*s("ДемоСтрока", 30)))


def run_m6(tree):
    five_objects(tree)
    own(tree, UNITS, "ДемоРеквизит", 20, "ForItem")
    add_section_attribute(tree, UNITS, "Сотрудники", section_attribute(*s("ДемоСтрока", 30)))
    # ten more objects (a subordinate catalog is not among them: its own attributes are refused, r6)
    remove_attribute(tree, SERVICES, "Идентификатор")
    flip(tree, PROJECTS, "РеквизитДопУпорядочивания", "IndexWithAdditionalOrder", "DontIndex")
    flip(tree, PAYROLL, "ПериодРегистрации", "IndexWithAdditionalOrder", "DontIndex")
    remove_attribute(tree, COMPONENTS, "Версия")
    widen(tree, COMPONENTS, "Идентификатор", 150, 200)
    widen(tree, NOTICES, "НаименованиеПользователя", 100, 200)
    flip(tree, NOTICES, "ВидОповещения", "DontIndex", "Index")
    widen(tree, CASH, "Выдать", 250, 300)
    widen(tree, PAYMENT, "НазначениеПлатежа", 210, 300)
    flip(tree, PAYMENT, "НомерВходящегоДокумента", "DontIndex", "Index")
    section_of(tree, LEAVE, "ДемоТЧ", [s("ДемоСтрока", 10)])
    own(tree, LEAVE)
    section_of(tree, INVOICE, "ДемоТЧ", [s("ДемоСтрока", 15), n("ДемоЧисло", 15, 2)])
    add_section_attribute(tree, INTAKE, "Товары", section_attribute(*s("ДемоСтрока", 10, "Index")))
    add_section_attribute(tree, CONTACTS, "КонтактнаяИнформация", section_attribute(*s("ДемоСтрока15", 15, "Index")), after="Вид")


def run_m7(tree):
    # new attributes that come indexed
    own(tree, KEYOPS, "ДемоИндексная", 30, "ForItem", indexing="Index")
    append_attribute(tree, KEYOPS, "ДемоЧисло", "Демо число", number(10, 2), "ForItem", indexing="Index")
    own(tree, BANKS, "ДемоИндексная", 30, "ForItem", indexing="Index")
    own(tree, BANKS, "ДемоИндексная2", 20, "ForFolderAndItem", indexing="Index")
    own(tree, JOBS, "ДемоПорядок", 20, "ForItem", indexing="IndexWithAdditionalOrder")
    own(tree, ORDER, "ДемоИндексная", 30, indexing="Index")
    append_attribute(tree, VOLUMES, "ДемоДата", "Демо дата", date("DateTime"), "ForItem", indexing="Index")


def run_r1(tree):
    remove_attribute(tree, KEYOPS, "Длительная")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20)])
    widen(tree, BANKS, "Город", 50, 200)
    section_of(tree, INVOICE, "ДемоТЧ", [s("ДемоСтрока", 15)])


def run_r2(tree):
    remove_attribute(tree, ORDER, "УдалитьЗаказЗакрыт")
    add_section_attribute(tree, ORDER, "СчетаНаОплату", section_attribute(*s("ДемоСтрока", 20)))
    flip(tree, VOLUMES, "МаксимальныйРазмер", "DontIndex", "Index")


def run_r3(tree):
    widen(tree, BANKS, "Город", 50, 200)
    remove_attribute(tree, JOBS, "Ключ")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20)])
    own(tree, ADOPTED)


def run_r4(tree):
    widen(tree, BANKS, "Город", 50, 200)
    remove_attribute(tree, JOBS, "Ключ")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20)])
    widen(tree, QUEUE, "ИмяПользователя", 32, 16)  # a shorter limit


def run_r5(tree):
    widen(tree, BANKS, "Город", 50, 200)
    remove_attribute(tree, JOBS, "Ключ")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20)])
    # a string becomes a number
    retype(tree, QUEUE, "ИмяПользователя", number(5, 0))


def run_r6(tree):
    widen(tree, BANKS, "Город", 50, 200)
    remove_attribute(tree, JOBS, "Ключ")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20)])
    # a subordinate catalog: the owner field is not covered when its own attributes change
    remove_attribute(tree, BANK_ACCOUNTS, "ГородБанка")
    widen(tree, BANK_ACCOUNTS, "НаименованиеБанка", 100, 150)


CASES = {
    "m1": run_m1, "m2": run_m2, "m3": run_m3, "m4": run_m4, "m5": run_m5, "m6": run_m6, "m7": run_m7,
    "r1": run_r1, "r2": run_r2, "r3": run_r3, "r4": run_r4, "r5": run_r5, "r6": run_r6,
}

if __name__ == "__main__":
    out, case = sys.argv[1], sys.argv[2]
    if case not in CASES:
        raise SystemExit("unknown case %s; known: %s" % (case, ", ".join(CASES)))
    tree = Tree(out)
    CASES[case](tree)
    tree.save()
