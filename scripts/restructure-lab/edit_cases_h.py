"""Case h of the ddl-track tree edits: attributes of every basic type, common-attribute position,
tabular section attribute, new tabular section (imported by edit_tree.py)."""
import os
import re
import uuid

import edit_tree as E
from edit_cases_more import child_block, insert_after_last, STRING, DECIMAL

T = lambda s: "<v8:Type>%s</v8:Type>" % s
DATE = lambda k: [T("xs:dateTime"), "<v8:DateQualifiers>", "\t<v8:DateFractions>%s</v8:DateFractions>" % k, "</v8:DateQualifiers>"]


def string_fixed(n):
    return [T("xs:string"), "<v8:StringQualifiers>", "\t<v8:Length>%d</v8:Length>" % n,
            "\t<v8:AllowedLength>Fixed</v8:AllowedLength>", "</v8:StringQualifiers>"]


COMPOSITE = [T("xs:string"), T("xs:decimal"), T("cfg:CatalogRef.Пользователи"),
             "<v8:NumberQualifiers>", "\t<v8:Digits>10</v8:Digits>", "\t<v8:FractionDigits>2</v8:FractionDigits>",
             "\t<v8:AllowedSign>Any</v8:AllowedSign>", "</v8:NumberQualifiers>",
             "<v8:StringQualifiers>", "\t<v8:Length>15</v8:Length>", "\t<v8:AllowedLength>Variable</v8:AllowedLength>",
             "</v8:StringQualifiers>"]

H1_ATTRS = [
    ("ДемоБулево", [T("xs:boolean")]),
    ("ДемоДатаВремя", DATE("DateTime")),
    ("ДемоДата", DATE("Date")),
    ("ДемоЧисло", DECIMAL(10, 0)),
    ("ДемоЧисло3", DECIMAL(12, 3)),
    ("ДемоСтрокаФикс", string_fixed(20)),
    ("ДемоСтрокаНеогр", STRING(0)),
    ("ДемоСсылкаСправочник", [T("cfg:CatalogRef.Пользователи")]),
    ("ДемоСсылкаПеречисление", [T("cfg:EnumRef.ТипыКонтактнойИнформации")]),
    ("ДемоСоставной", COMPOSITE),
    ("ДемоХранилище", [T("v8:ValueStorage")]),
    ("ДемоУИД", [T("v8:UUID")]),
]


def case_h():
    # h1: many types on a flat catalog with data
    p = os.path.join("Catalogs", "КлючевыеОперации.xml")
    E.save_orig("h", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    for name, tx in H1_ATTRS:
        block = child_block("Attribute", name, name, tx, nl, use=True)
        txt = insert_after_last(txt, "Attribute", block, nl)
    E.write(p, txt, bom)
    E.save_new("h", p)
    print("h1: %d attributes on %s" % (len(H1_ATTRS), p))

    # h2: one attribute on a hierarchical catalog that carries common-attribute fields
    p = os.path.join("Catalogs", "ВидыКонтактнойИнформации.xml")
    E.save_orig("h", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    block = child_block("Attribute", "ДемоТестСтрока", "Демо тест строка", STRING(30), nl, use=True)
    txt = insert_after_last(txt, "Attribute", block, nl)
    E.write(p, txt, bom)
    E.save_new("h", p)
    print("h2: attribute on", p)

    # h3: an attribute in an existing tabular section
    p = os.path.join("Catalogs", "_ДемоПартнеры.xml")
    E.save_orig("h", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    m = re.search(r'\t\t\t<TabularSection uuid="[^"]+">(?:(?!\n\t\t\t</TabularSection>).)*?<Name>КонтактнаяИнформация</Name>.*?\n\t\t\t</TabularSection>', txt, re.S)
    assert m, "tabular section КонтактнаяИнформация"
    ts = m.group(0)
    close = nl + "\t\t\t\t</ChildObjects>"
    assert ts.count(close) == 1
    blk = child_block("Attribute", "ДемоТабличныйРеквизит", "Демо табличный реквизит", STRING(15), nl, no_fill=True)
    blk = nl.join(("\t\t" + x) if x else x for x in blk.split(nl))     # two more tabs
    ts2 = ts.replace(close, nl + blk + close)
    txt = txt[:m.start()] + ts2 + txt[m.end():]
    E.write(p, txt, bom)
    E.save_new("h", p)
    print("h3: tabular section attribute on", p)

    # h4: a new tabular section on another catalog (cloned from its smallest section)
    p = os.path.join("Catalogs", "_ДемоКонтрагенты.xml")
    E.save_orig("h", p)
    txt, bom = E.read(p)
    nl = E.nl_of(txt)
    secs = list(re.finditer(r'\t\t\t<TabularSection uuid="[^"]+">.*?\n\t\t\t</TabularSection>', txt, re.S))
    src = min(secs, key=lambda m: len(m.group(0)))
    body = src.group(0)
    mapping = {}

    def newid(m):
        u = m.group(0)
        mapping.setdefault(u, str(uuid.uuid4()))
        return mapping[u]

    body = re.sub(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", newid, body)
    old_name = re.search(r"<Name>([^<]+)</Name>", body).group(1)
    body = body.replace("Contragenty", "Contragenty")
    body = body.replace("._ДемоКонтрагенты.%s" % old_name, "._ДемоКонтрагенты.ДемоНоваяТЧ")
    body = body.replace("<Name>%s</Name>" % old_name, "<Name>ДемоНоваяТЧ</Name>", 1)
    body = body.replace("<v8:content>%s</v8:content>" % old_name, "<v8:content>Демо новая ТЧ</v8:content>", 1)
    last = secs[-1]
    txt = txt[:last.end()] + nl + body + txt[last.end():]
    E.write(p, txt, bom)
    E.save_new("h", p)
    print("h4: new tabular section (clone of %s) on %s" % (old_name, p))
