"""Cases b..g of the ddl-track tree edits (imported by edit_tree.py)."""
import os
import re
import uuid

import edit_tree as E


def child_block(kind, name, synonym, type_xml, nl, extra_before_indexing=(), extra_after=(), use=False,
                fill_value='<FillValue xsi:nil="true"/>', fill_checking="DontCheck", indexing="DontIndex",
                no_fill=False):
    """<Attribute|Dimension|Resource> block at object level (3 tabs), 2.20 export layout."""
    lines = [
        '<%s uuid="%s">' % (kind, uuid.uuid4()),
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
    ] + ([] if no_fill else [
        "\t\t<FillFromFillingValue>false</FillFromFillingValue>",
        "\t\t" + fill_value,
    ]) + [
        "\t\t<FillChecking>%s</FillChecking>" % fill_checking,
        "\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>",
        "\t\t<ChoiceParameterLinks/>",
        "\t\t<ChoiceParameters/>",
        "\t\t<QuickChoice>Auto</QuickChoice>",
        "\t\t<CreateOnInput>Auto</CreateOnInput>",
        "\t\t<ChoiceForm/>",
        "\t\t<LinkByType/>",
        "\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>",
    ] + ["\t\t" + x for x in extra_before_indexing] + (["\t\t<Use>ForItem</Use>"] if use else []) + [
        "\t\t<Indexing>%s</Indexing>" % indexing,
        "\t\t<FullTextSearch>Use</FullTextSearch>",
        "\t\t<DataHistory>Use</DataHistory>",
    ] + ["\t\t" + x for x in extra_after] + [
        "\t</Properties>",
        "</%s>" % kind,
    ]
    return nl.join(("\t\t\t" + x) for x in lines)


def STRING(n):
    return ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>%d</v8:Length>" % n,
            "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]


def DECIMAL(d, f):
    return ["<v8:Type>xs:decimal</v8:Type>", "<v8:NumberQualifiers>", "\t<v8:Digits>%d</v8:Digits>" % d,
            "\t<v8:FractionDigits>%d</v8:FractionDigits>" % f, "\t<v8:AllowedSign>Any</v8:AllowedSign>",
            "</v8:NumberQualifiers>"]


def insert_after_last(txt, close_tag, block, nl):
    """Insert `block` after the last object-level (3 tabs) </close_tag>."""
    needle = nl + "\t\t\t</%s>" % close_tag
    i = txt.rfind(needle)
    assert i >= 0, close_tag
    j = i + len(needle)
    return txt[:j] + nl + block + txt[j:]


def case_b():
    """Number(15,2) attribute added to Document._ДемоЗаказПокупателя."""
    p = os.path.join("Documents", "_ДемоЗаказПокупателя.xml")
    E.save_orig("b", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    block = child_block("Attribute", "ДемоНовоеЧисло", "Демо новое число", DECIMAL(15, 2), nl)
    txt = insert_after_last(txt, "Attribute", block, nl)
    E.write(p, txt, bom)
    E.save_new("b", p)
    print("case b: uuid", re.search(r'<Attribute uuid="([^"]+)">\s*<Properties>\s*<Name>ДемоНовоеЧисло', txt).group(1))


def case_c():
    """A new catalog ДемоНовыйСправочник cloned from the attribute-less Удалить_ДемоОбщиеСведения."""
    src = os.path.join("Catalogs", "Удалить_ДемоОбщиеСведения.xml")
    dst = os.path.join("Catalogs", "ДемоНовыйСправочник.xml")
    txt, bom = E.read(src)
    mapping = {}

    def newid(m):
        u = m.group(0)
        if u not in mapping:
            mapping[u] = str(uuid.uuid4())
        return mapping[u]

    txt = re.sub(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", newid, txt)
    txt = txt.replace("Удалить_ДемоОбщиеСведения", "ДемоНовыйСправочник")
    txt = txt.replace("(не используется) Демо: Общие сведения", "Демо: Новый справочник")
    E.write(dst, txt, bom)
    E.save_new("c", dst)
    cp = "Configuration.xml"
    E.save_orig("c", cp)
    ctxt, cbom = E.read(cp)
    cnl = E.nl_of(ctxt)
    names_ = re.findall(r"<Catalog>([^<]+)</Catalog>", ctxt)
    later = [n for n in names_ if n > "ДемоНовыйСправочник"]
    if later:
        key = "\t\t\t<Catalog>%s</Catalog>" % later[0]
        ctxt = ctxt.replace(key, "\t\t\t<Catalog>ДемоНовыйСправочник</Catalog>" + cnl + key, 1)
    else:
        last = "\t\t\t<Catalog>%s</Catalog>" % names_[-1]
        ctxt = ctxt.replace(last, last + cnl + "\t\t\t<Catalog>ДемоНовыйСправочник</Catalog>", 1)
    E.write(cp, ctxt, cbom)
    E.save_new("c", cp)
    print("case c: catalog uuid", re.search(r'<Catalog uuid="([^"]+)"', txt).group(1), "inserted before", later[:1])


def case_d():
    """A dimension (String 10) added to InformationRegister._ДемоЦеныНоменклатуры and a resource
    (Number 15,2) to AccumulationRegister._ДемоОстаткиТоваровВМестахХранения."""
    p = os.path.join("InformationRegisters", "_ДемоЦеныНоменклатуры.xml")
    E.save_orig("d", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    block = child_block("Dimension", "ДемоНовоеИзмерение", "Демо новое измерение", STRING(10), nl,
                        extra_before_indexing=["<Master>false</Master>", "<MainFilter>false</MainFilter>",
                                               "<DenyIncompleteValues>false</DenyIncompleteValues>"],
                        extra_after=["<TypeReductionMode>TransformValues</TypeReductionMode>"])
    txt = insert_after_last(txt, "Dimension", block, nl)
    E.write(p, txt, bom)
    E.save_new("d", p)
    print("case d: dimension uuid", re.search(r'<Dimension uuid="([^"]+)">\s*<Properties>\s*<Name>ДемоНовоеИзмерение', txt).group(1))
    p = os.path.join("AccumulationRegisters", "_ДемоОстаткиТоваровВМестахХранения.xml")
    E.save_orig("d", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    block = child_block("Resource", "ДемоНовыйРесурс", "Демо новый ресурс", DECIMAL(15, 2), nl)
    txt = insert_after_last(txt, "Resource", block, nl)
    E.write(p, txt, bom)
    E.save_new("d", p)
    print("case d: resource uuid", re.search(r'<Resource uuid="([^"]+)">\s*<Properties>\s*<Name>ДемоНовыйРесурс', txt).group(1))


PARTNERS = os.path.join("Catalogs", "_ДемоПартнеры.xml")


def edit_new_attr(case, fn):
    E.save_orig(case, PARTNERS)
    txt, bom = E.read(PARTNERS)
    m = re.search(r'<Attribute uuid="[^"]+">\s*<Properties>\s*<Name>ДемоНовыйРеквизит</Name>.*?</Attribute>', txt, re.S)
    assert m, "the attribute added in case a is not there"
    new = fn(m.group(0))
    txt = txt[:m.start()] + new + txt[m.end():]
    E.write(PARTNERS, txt, bom)
    E.save_new(case, PARTNERS)


def case_e():
    """String length of an existing attribute changed: ДемоНовыйРеквизит 50 -> 100 (data conversion)."""
    edit_new_attr("e", lambda b: b.replace("<v8:Length>50</v8:Length>", "<v8:Length>100</v8:Length>"))
    print("case e: 50 -> 100")


def case_g():
    """Index flag switched on for ДемоНовыйРеквизит (Indexing DontIndex -> Index)."""
    edit_new_attr("g", lambda b: b.replace("<Indexing>DontIndex</Indexing>", "<Indexing>Index</Indexing>"))
    print("case g: Indexing=Index")


def case_f():
    """The attribute ДемоНовыйРеквизит (indexed, with data) deleted."""
    E.save_orig("f", PARTNERS)
    txt, bom = E.read(PARTNERS)
    nl = E.nl_of(txt)
    m = re.search(r'\t\t\t<Attribute uuid="[^"]+">\s*<Properties>\s*<Name>ДемоНовыйРеквизит</Name>.*?</Attribute>' + nl, txt, re.S)
    assert m
    txt = txt[:m.start()] + txt[m.end():]
    E.write(PARTNERS, txt, bom)
    E.save_new("f", PARTNERS)
    print("case f: attribute deleted")


MORE = {"b": case_b, "c": case_c, "d": case_d, "e": case_e, "g": case_g, "f": case_f}
