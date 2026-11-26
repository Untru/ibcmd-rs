"""Case 2 edits of the local copy of the reference tree.

usage: python make_case2.py <tree> attr    add a String(25) attribute to catalog _ДемоВидыНоменклатуры
       python make_case2.py <tree> newcat  add a small new catalog ТрассаНовыйСправочник (and list it in Configuration.xml)
       python make_case2.py <tree> revert <reference tree>   restore the touched files from the reference tree

Byte-safe: BOM and line endings are kept (the sources are UTF-8 with BOM, LF inside the XML files).
"""
import os
import re
import shutil
import sys
import uuid

tree = sys.argv[1]
mode = sys.argv[2]
CAT = "_ДемоВидыНоменклатуры"
NEW = "ТрассаНовыйСправочник"
ATTR = "ТрассаРеквизит"


def read(path):
    with open(path, "rb") as fh:
        raw = fh.read()
    bom = raw.startswith(b"\xef\xbb\xbf")
    text = raw[3:].decode("utf-8") if bom else raw.decode("utf-8")
    return text, bom


def write(path, text, bom):
    with open(path, "wb") as fh:
        fh.write((b"\xef\xbb\xbf" if bom else b"") + text.encode("utf-8"))


def new_uuid():
    return str(uuid.uuid4())


if mode == "attr":
    path = os.path.join(tree, "Catalogs", CAT + ".xml")
    text, bom = read(path)
    nl = "\r\n" if "\r\n" in text else "\n"
    # copy the boolean attribute block and turn it into String(25)
    m = re.search(r'\t\t\t<Attribute uuid="[0-9a-f-]+">(?:(?!</Attribute>).)*?<v8:Type>xs:boolean</v8:Type>.*?</Attribute>' + nl, text, re.S)
    assert m, "no boolean attribute to copy"
    block = m.group(0)
    block = re.sub(r'<Attribute uuid="[0-9a-f-]+">', f'<Attribute uuid="{new_uuid()}">', block, count=1)
    block = re.sub(r"<Name>[^<]*</Name>", f"<Name>{ATTR}</Name>", block, count=1)
    block = re.sub(r"(<Synonym>.*?<v8:content>)[^<]*(</v8:content>)", r"\g<1>Трасса: реквизит\g<2>", block, count=1, flags=re.S)
    block = re.sub(r"(<ToolTip>.*?<v8:content>)[^<]*(</v8:content>)", r"\g<1>Реквизит для трассы применения\g<2>", block, count=1, flags=re.S)
    block = block.replace(
        "<v8:Type>xs:boolean</v8:Type>",
        "<v8:Type>xs:string</v8:Type>" + nl + "\t\t\t\t\t\t<v8:StringQualifiers>" + nl + "\t\t\t\t\t\t\t<v8:Length>25</v8:Length>" + nl
        + "\t\t\t\t\t\t\t<v8:AllowedLength>Variable</v8:AllowedLength>" + nl + "\t\t\t\t\t\t</v8:StringQualifiers>",
    )
    block = block.replace('<FillValue xsi:nil="true"/>', '<FillValue xsi:type="xs:string"/>')
    # insert before the first <Form> child (after the last attribute)
    idx = text.index("\t\t\t<Form>", text.index("<ChildObjects>"))
    text = text[:idx] + block + text[idx:]
    write(path, text, bom)
    print("added attribute", ATTR, "String(25) to", CAT)
elif mode == "newcat":
    src = os.path.join(tree, "Catalogs", CAT + ".xml")
    text, bom = read(src)
    nl = "\r\n" if "\r\n" in text else "\n"
    # root uuid and generated type ids
    text = re.sub(r'<Catalog uuid="[0-9a-f-]+">', f'<Catalog uuid="{new_uuid()}">', text, count=1)
    text = re.sub(r"<xr:TypeId>[0-9a-f-]+</xr:TypeId>", lambda m: f"<xr:TypeId>{new_uuid()}</xr:TypeId>", text)
    text = re.sub(r"<xr:ValueId>[0-9a-f-]+</xr:ValueId>", lambda m: f"<xr:ValueId>{new_uuid()}</xr:ValueId>", text)
    text = text.replace(CAT, NEW)
    text = re.sub(r"<v8:content>Демо: Виды номенклатуры</v8:content>", "<v8:content>Трасса: новый справочник</v8:content>", text)
    # drop the attributes and the form references
    cs = text.index("<ChildObjects>")
    ce = text.index("</ChildObjects>") + len("</ChildObjects>")
    text = text[:cs] + "<ChildObjects/>" + text[ce:]
    text = re.sub(r"<DefaultObjectForm>[^<]*</DefaultObjectForm>", "<DefaultObjectForm/>", text)
    text = re.sub(r"<(Default\w*Form|Auxiliary\w*Form)>[^<]*</\1>", r"<\1/>", text)
    write(os.path.join(tree, "Catalogs", NEW + ".xml"), text, bom)
    # register in Configuration.xml
    cfg = os.path.join(tree, "Configuration.xml")
    ctext, cbom = read(cfg)
    anchor = f"<Catalog>{CAT}</Catalog>"
    assert anchor in ctext, "catalog not listed in Configuration.xml"
    ctext = ctext.replace(anchor, anchor + nl + "\t\t\t" + f"<Catalog>{NEW}</Catalog>", 1)
    write(cfg, ctext, cbom)
    print("added catalog", NEW, "(from", CAT + ")")
elif mode == "revert":
    ref = sys.argv[3]
    for rel in (os.path.join("Catalogs", CAT + ".xml"), "Configuration.xml"):
        shutil.copyfile(os.path.join(ref, rel), os.path.join(tree, rel))
    p = os.path.join(tree, "Catalogs", NEW + ".xml")
    if os.path.exists(p):
        os.remove(p)
    print("reverted")
else:
    raise SystemExit("mode: attr | newcat | revert")
