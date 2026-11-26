"""S1-E (issue #401): a new tabular section, an attribute of an existing tabular section, on the pristine БСП.

Reads the reference native export (never written), writes the edited files - only those - under
<out>/stage/<relative path> and the originals under <out>/before/, ready for
`import_files.ps1 -BaseDir <out>\\stage -Files ...` (a native `import files --partial`), as edit_cases_s2.py does.

usage: python edit_cases_s3.py <out dir> <case>
  e1  new tabular sections: a flat catalog, a hierarchical one, a catalog that has a section, a document, a document that has one
  e3  attributes of existing tabular sections: hierarchical and flat catalogs, a document; first / middle / last place; two at once
  e4  nested numbering: own attributes, new sections and attributes of an existing section in the same objects (a document, a catalog)
  e5  the shapes of e1 on objects no extension of the БСП clone adopts (S1-I refuses an adopted object): a flat catalog, a hierarchical one,
      a subordinate one, two documents
  e6  the shapes of e3 on such objects: a hierarchical catalog, a subordinate one with indexed attributes first / in the middle of a section
      that has indexed attributes already, a document
"""
import os
import re
import sys
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from edit_cases_s1 import (BOOLEAN, REF, date, nl_of, number, string_var)  # noqa: E402
from edit_cases_s2 import Tree, append_attribute  # noqa: E402

UNLIMITED = ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>0</v8:Length>",
             "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


def section_attribute(name, synonym, type_xml, indexing="DontIndex", tabs=5):
    """An <Attribute> of a tabular section as the 2.20 export writes it (no Use, no filling value)."""
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
        "\t\t<FillChecking>DontCheck</FillChecking>",
        "\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>",
        "\t\t<ChoiceParameterLinks/>",
        "\t\t<ChoiceParameters/>",
        "\t\t<QuickChoice>Auto</QuickChoice>",
        "\t\t<CreateOnInput>Auto</CreateOnInput>",
        "\t\t<ChoiceForm/>",
        "\t\t<LinkByType/>",
        "\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>",
        "\t\t<Indexing>%s</Indexing>" % indexing,
        "\t\t<FullTextSearch>Use</FullTextSearch>",
        "\t\t<DataHistory>Use</DataHistory>",
        "\t</Properties>",
        "</Attribute>",
    ]
    return lines, tabs


def render(attribute, nl):
    lines, tabs = attribute
    return nl.join(("\t" * tabs + x) for x in lines)


def read_ref(rel):
    with open(os.path.join(REF, rel), "rb") as f:
        return f.read().decode("utf-8-sig")


def section_span(text, name):
    """(start, end) of the <TabularSection> named `name` (object level, 3 tabs), the end after its closing tag."""
    nl = nl_of(text)
    for m in re.finditer(re.escape(nl + "\t\t\t<TabularSection uuid=\""), text):
        end = text.find(nl + "\t\t\t</TabularSection>", m.start())
        assert end > 0
        block = text[m.start():end]
        if re.search(r"<Name>%s</Name>" % re.escape(name), block):
            return m.start(), end + len(nl + "\t\t\t</TabularSection>")
    raise SystemExit("no tabular section named %s" % name)


def new_section(tree, rel, kind_prefix, name, synonym, attributes, template_rel, template_name):
    """Adds a section (cloned from a section of `template_rel`, all ids new, the attributes replaced) after the last section of
    the object, or after its last own attribute when it has none."""
    text = tree.get(rel)
    nl = nl_of(text)
    obj = re.search(r"<Name>([^<]+)</Name>", text).group(1)
    template = read_ref(template_rel)
    start, end = section_span(template, template_name)
    block = template[start + len(nl_of(template)):end]
    old_obj = re.search(r"<Name>([^<]+)</Name>", template).group(1)
    mapping = {}
    block = UUID.sub(lambda m: mapping.setdefault(m.group(0), str(uuid.uuid4())), block)
    block = block.replace("%sTabularSection.%s.%s" % (kind_prefix, old_obj, template_name),
                          "%sTabularSection.%s.%s" % (kind_prefix, obj, name))
    block = block.replace("%sTabularSectionRow.%s.%s" % (kind_prefix, old_obj, template_name),
                          "%sTabularSectionRow.%s.%s" % (kind_prefix, obj, name))
    block = block.replace("<Name>%s</Name>" % template_name, "<Name>%s</Name>" % name, 1)
    block = re.sub(r"(<Synonym>.*?<v8:content>)[^<]*(</v8:content>)", lambda m: m.group(1) + synonym + m.group(2), block,
                   count=1, flags=re.S)
    block = re.sub(r"<Comment>[^<]*</Comment>", "<Comment/>", block, count=1)
    block = re.sub(r"<ToolTip>.*?</ToolTip>", "<ToolTip/>", block, count=1, flags=re.S)
    # the attributes: the section's own <ChildObjects> is the last one of the block
    i = block.rfind(nl + "\t\t\t\t<ChildObjects>")
    assert i > 0
    body = nl.join(render(section_attribute(*a), nl) for a in attributes)
    block = block[:i] + nl + "\t\t\t\t<ChildObjects>" + nl + body + nl + "\t\t\t\t</ChildObjects>" + nl + "\t\t\t</TabularSection>"
    sections = list(re.finditer(re.escape(nl + "\t\t\t</TabularSection>"), text))
    if sections:
        at = sections[-1].end()
    else:
        at = text.rfind(nl + "\t\t\t</Attribute>") + len(nl + "\t\t\t</Attribute>")
        assert at > len(nl + "\t\t\t</Attribute>")
    tree.put(rel, text[:at] + nl + block + text[at:])


def add_section_attribute(tree, rel, section, attribute, after=None):
    """An attribute in an existing section: at the end, first (`after` = '') or after the attribute named `after`."""
    text = tree.get(rel)
    nl = nl_of(text)
    start, end = section_span(text, section)
    block = text[start:end]
    new = render(attribute, nl)
    if after is None:
        close = nl + "\t\t\t\t</ChildObjects>"
        assert block.count(close) == 1
        block = block.replace(close, nl + new + close)
    elif after == "":
        opening = "<ChildObjects>"
        i = block.rfind(opening)
        j = i + len(opening)
        block = block[:j] + nl + new + block[j:]
    else:
        m = re.search(r"<Name>%s</Name>" % re.escape(after), block[block.rfind("<ChildObjects>"):])
        assert m, after
        base = block.rfind("<ChildObjects>")
        close = block.find(nl + "\t\t\t\t\t</Attribute>", base + m.start())
        j = close + len(nl + "\t\t\t\t\t</Attribute>")
        block = block[:j] + nl + new + block[j:]
    tree.put(rel, text[:start] + block + text[end:])


C_HIER = ("Catalogs/_ДемоПодразделения.xml", "Сотрудники")  # hierarchical catalog: the section has <Use>
C_FLAT = ("Catalogs/НастройкиТранспортаСообщенийОбмена.xml", "Настройки")
D_TPL = ("Documents/_ДемоОприходованиеТоваров.xml", "Товары")
CATALOG = "Catalog"
DOCUMENT = "Document"


def hierarchical(tree, rel):
    return "<Hierarchical>true</Hierarchical>" in tree.get(rel)


def add_section(tree, rel, name, synonym, attributes):
    kind = DOCUMENT if rel.startswith("Documents/") else CATALOG
    if kind == DOCUMENT:
        template = D_TPL
    else:
        template = C_HIER if hierarchical(tree, rel) else C_FLAT
    new_section(tree, rel, kind, name, synonym, attributes, template[0], template[1])


def s(name, size, indexing="DontIndex"):
    return (name, name, string_var(size), indexing)


def n(name, digits, fraction, indexing="DontIndex"):
    return (name, name, number(digits, fraction), indexing)


def run_e1(tree):
    add_section(tree, "Catalogs/_ДемоСтавкиНДС.xml", "ДемоТЧ", "Демо ТЧ",
                [s("ДемоСтрока", 20), n("ДемоЧисло", 10, 2, "Index"), ("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex"),
                 ("ДемоТекст", "ДемоТекст", UNLIMITED, "DontIndex")])
    add_section(tree, "Catalogs/_ДемоМестаХранения.xml", "ДемоТЧ", "Демо ТЧ",
                [("ДемоДата", "ДемоДата", date("Date"), "DontIndex"), s("ДемоСтрока", 50, "Index")])
    add_section(tree, "Catalogs/_ДемоКонтактныеЛицаПартнеров.xml", "ДемоТЧ2", "Демо ТЧ 2", [s("ДемоСтрока", 10)])
    add_section(tree, "Documents/_ДемоСчетФактураПолученный.xml", "ДемоТЧ", "Демо ТЧ",
                [s("ДемоСтрока", 15), n("ДемоЧисло", 15, 2)])
    add_section(tree, "Documents/_ДемоСписаниеТоваров.xml", "ДемоТЧ2", "Демо ТЧ 2",
                [("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex")])


def run_e3(tree):
    add_section_attribute(tree, "Catalogs/_ДемоПодразделения.xml", "Сотрудники", section_attribute(*s("ДемоСтрока", 30)))
    for attribute in (section_attribute(*n("ДемоЧисло", 12, 3)),
                      section_attribute("ДемоФлаг", "ДемоФлаг", BOOLEAN)):
        add_section_attribute(tree, "Catalogs/_ДемоОрганизации.xml", "КонтактнаяИнформация", attribute)
    add_section_attribute(tree, "Documents/_ДемоОприходованиеТоваров.xml", "Товары",
                          section_attribute(*n("ДемоЧисло", 15, 3)), after="")
    add_section_attribute(tree, "Documents/_ДемоОприходованиеТоваров.xml", "Товары",
                          section_attribute(*s("ДемоСтрока", 10, "Index")))
    add_section_attribute(tree, "Catalogs/_ДемоПартнеры.xml", "КонтактнаяИнформация",
                          section_attribute(*s("ДемоСтрока15", 15)), after="Вид")


def run_e4(tree):
    doc = "Documents/_ДемоОтпускаСотрудников.xml"
    append_attribute(tree, doc, "ДемоРеквизит", "Демо реквизит", string_var(20))
    add_section(tree, doc, "ДемоТЧ1", "Демо ТЧ 1", [s("ДемоСтрока", 10), n("ДемоЧисло", 10, 0)])
    add_section_attribute(tree, doc, "Сотрудники", section_attribute(*s("ДемоСтрока5", 5)))
    add_section(tree, doc, "ДемоТЧ2", "Демо ТЧ 2", [("ДемоДата", "ДемоДата", date("Date"), "DontIndex")])
    cat = "Catalogs/_ДемоПодразделения.xml"
    append_attribute(tree, cat, "ДемоРеквизит", "Демо реквизит", string_var(20), use="ForItem")
    add_section_attribute(tree, cat, "Сотрудники", section_attribute(*s("ДемоСтрока2", 12)))
    add_section(tree, cat, "ДемоТЧ", "Демо ТЧ", [("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex")])


def run_e5(tree):
    add_section(tree, "Catalogs/_ДемоСтавкиНДС.xml", "ДемоТЧ", "Демо ТЧ",
                [s("ДемоСтрока", 20), n("ДемоЧисло", 10, 2, "Index"), ("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex"),
                 ("ДемоТекст", "ДемоТекст", UNLIMITED, "DontIndex")])
    add_section(tree, "Catalogs/_ДемоПодразделения.xml", "ДемоТЧ", "Демо ТЧ",
                [("ДемоДата", "ДемоДата", date("Date"), "DontIndex"), s("ДемоСтрока", 50, "Index")])
    add_section(tree, "Catalogs/_ДемоКонтактныеЛицаПартнеров.xml", "ДемоТЧ2", "Демо ТЧ 2", [s("ДемоСтрока", 10)])
    add_section(tree, "Documents/_ДемоСчетФактураПолученный.xml", "ДемоТЧ", "Демо ТЧ",
                [s("ДемоСтрока", 15), n("ДемоЧисло", 15, 2)])
    add_section(tree, "Documents/_ДемоОприходованиеТоваров.xml", "ДемоТЧ2", "Демо ТЧ 2",
                [("ДемоФлаг", "ДемоФлаг", BOOLEAN, "DontIndex")])


def run_e6(tree):
    add_section_attribute(tree, "Catalogs/_ДемоПодразделения.xml", "Сотрудники", section_attribute(*s("ДемоСтрока", 30)))
    contacts = "Catalogs/_ДемоКонтактныеЛицаПартнеров.xml"
    # the section has Тип and Вид indexed already: a new indexed attribute first, one in the middle, a plain one last
    add_section_attribute(tree, contacts, "КонтактнаяИнформация", section_attribute(*n("ДемоЧисло", 12, 3, "Index")), after="")
    add_section_attribute(tree, contacts, "КонтактнаяИнформация", section_attribute(*s("ДемоСтрока15", 15, "Index")), after="Вид")
    add_section_attribute(tree, contacts, "КонтактнаяИнформация", section_attribute("ДемоФлаг", "ДемоФлаг", BOOLEAN))
    add_section_attribute(tree, "Documents/_ДемоОприходованиеТоваров.xml", "Товары",
                          section_attribute(*n("ДемоЧисло", 15, 3)), after="")
    add_section_attribute(tree, "Documents/_ДемоОприходованиеТоваров.xml", "Товары",
                          section_attribute(*s("ДемоСтрока", 10, "Index")))


CASES = {"e1": run_e1, "e3": run_e3, "e4": run_e4, "e5": run_e5, "e6": run_e6}

if __name__ == "__main__":
    out, case = sys.argv[1], sys.argv[2]
    if case not in CASES:
        raise SystemExit("unknown case %s; known: %s" % (case, ", ".join(CASES)))
    tree = Tree(out)
    CASES[case](tree)
    tree.save()
