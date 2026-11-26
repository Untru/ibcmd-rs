"""Case a on the 8.5 БСП: a String(50) attribute in Catalog._ДемоПартнеры, written next to the reference 8.5 export.

usage: python edit85.py <8.5 native export> <output folder>
Writes <output folder>/Catalogs/_ДемоПартнеры.xml only (for `ibcmd infobase config import files --partial`), with the
attribute uuid c60cdc87-198a-4f6e-8f17-76bcb1b1914b (the one of case a2 on 8.3.27, so the two traces compare).
"""
import os
import sys

from edit_tree import attr_xml

UUID = "c60cdc87-198a-4f6e-8f17-76bcb1b1914b"


def main():
    tree, out = sys.argv[1], sys.argv[2]
    rel = os.path.join("Catalogs", "_ДемоПартнеры.xml")
    with open(os.path.join(tree, rel), "rb") as f:
        raw = f.read()
    bom = raw.startswith(b"\xef\xbb\xbf")
    txt = raw.decode("utf-8-sig")
    nl = "\r\n" if "\r\n" in txt else "\n"
    type_xml = ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>50</v8:Length>",
                "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]
    block = attr_xml("ДемоНовыйРеквизит", "Демо новый реквизит", type_xml, nl, 3)
    import re
    block = re.sub(r'<Attribute uuid="[^"]+">', '<Attribute uuid="%s">' % UUID, block, count=1)
    marker = "\t\t\t<TabularSection uuid=\"be5a03cb-a98b-4e33-8699-623c01a231a1\">"
    assert txt.count(marker) == 1, "marker"
    txt = txt.replace(marker, block + nl + marker)
    dst = os.path.join(out, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "wb") as f:
        f.write((b"\xef\xbb\xbf" if bom else b"") + txt.encode("utf-8"))
    print("wrote", dst, len(txt))


if __name__ == "__main__":
    main()
