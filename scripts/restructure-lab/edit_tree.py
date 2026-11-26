"""Edits of the working XML tree (lab/04/restructure/tree/bsp8327) for the ddl-track cases.

usage: python edit_tree.py <case>      cases: a b c d e f g
Every edit is applied to the working tree in place (cumulative) and the files it touched are
copied to tree/patches/<case>/ (original, before, and edited, after) for the record.
Files use CRLF? -> the tree is LF/CRLF as exported; edits keep the file's own newline style.
"""
import os
import re
import shutil
import sys
import uuid

from lab import TREE
from lab import ROOT as _ROOT
PATCH = os.path.join(_ROOT, "tree", "patches")


def read(p):
    with open(os.path.join(TREE, p), "rb") as f:
        raw = f.read()
    bom = raw.startswith(b"\xef\xbb\xbf")
    txt = raw.decode("utf-8-sig")
    return txt, bom


def write(p, txt, bom):
    with open(os.path.join(TREE, p), "wb") as f:
        f.write((b"\xef\xbb\xbf" if bom else b"") + txt.encode("utf-8"))


def save_orig(case, p):
    dst = os.path.join(PATCH, case, "before", p)
    if not os.path.exists(dst):
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(os.path.join(TREE, p), dst)


def save_new(case, p):
    dst = os.path.join(PATCH, case, "after", p)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(os.path.join(TREE, p), dst)


def nl_of(txt):
    return "\r\n" if "\r\n" in txt else "\n"


def attr_xml(name, synonym, type_xml, nl, tabs, extra_use=True, indexing="DontIndex", fill_checking="DontCheck"):
    """A catalog/document attribute as the 2.20 export writes it (fill value absent)."""
    t = "\t" * tabs
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
        "\t\t<FillValue/>" if False else '\t\t<FillValue xsi:nil="true"/>',
        "\t\t<FillChecking>%s</FillChecking>" % fill_checking,
        "\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>",
        "\t\t<ChoiceParameterLinks/>",
        "\t\t<ChoiceParameters/>",
        "\t\t<QuickChoice>Auto</QuickChoice>",
        "\t\t<CreateOnInput>Auto</CreateOnInput>",
        "\t\t<ChoiceForm/>",
        "\t\t<LinkByType/>",
        "\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>",
    ] + (["\t\t<Use>ForItem</Use>"] if extra_use else []) + [
        "\t\t<Indexing>%s</Indexing>" % indexing,
        "\t\t<FullTextSearch>Use</FullTextSearch>",
        "\t\t<DataHistory>Use</DataHistory>",
        "\t</Properties>",
        "</Attribute>",
    ]
    return nl.join((t + x) for x in lines)


def case_a():
    """String(50) attribute added to Catalog._ДемоПартнеры (top level, after ВидПартнера)."""
    p = os.path.join("Catalogs", "_ДемоПартнеры.xml")
    save_orig("a", p)
    txt, bom = read(p)
    nl = nl_of(txt)
    type_xml = ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>50</v8:Length>",
                "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]
    block = attr_xml("ДемоНовыйРеквизит", "Демо новый реквизит", type_xml, nl, 3)
    marker = "\t\t\t<TabularSection uuid=\"be5a03cb-a98b-4e33-8699-623c01a231a1\">"
    assert txt.count(marker) == 1
    txt = txt.replace(marker, block + nl + marker)
    write(p, txt, bom)
    save_new("a", p)
    m = re.search(r'<Attribute uuid="([^"]+)">\s*<Properties>\s*<Name>ДемоНовыйРеквизит', txt)
    print("case a: attribute uuid", m.group(1))


CASES = {"a": case_a}

if __name__ == "__main__":
    case = sys.argv[1]
    if case in CASES:
        CASES[case]()
    elif case == "h":
        import edit_cases_h
        edit_cases_h.case_h()
    elif case == "k":
        import edit_cases_k
        edit_cases_k.case_k()
    else:
        import edit_cases_more
        edit_cases_more.MORE[case]()
