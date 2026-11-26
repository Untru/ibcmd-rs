"""One more new String attribute in Catalog._ДемоПартнеры, on top of a catalog file (the ALTER TABLE experiment).

usage: python edit_second.py [<input catalog xml> <output folder> <name> <synonym> <length> <uuid>]
Default: reads <lab>/tree/patches/a/after/Catalogs/_ДемоПартнеры.xml (case a: ДемоНовыйРеквизит) and writes
<lab>/tree_b/Catalogs/_ДемоПартнеры.xml with a String(20) attribute ДемоВторойРеквизит after it. The output is meant
for `ibcmd infobase config import files --partial` (import_files.ps1); run it again on tree_b's file for a third.
"""
import os
import re
import sys

from edit_tree import attr_xml
from lab import ROOT


def main():
    rel = os.path.join("Catalogs", "_ДемоПартнеры.xml")
    args = sys.argv[1:]
    if args:
        source, out_dir, name, synonym, length, uuid = args
    else:
        source = os.path.join(ROOT, "tree", "patches", "a", "after", rel)
        out_dir = os.path.join(ROOT, "tree_b")
        name, synonym, length, uuid = "ДемоВторойРеквизит", "Демо второй реквизит", "20", "7b0e2f4c-91c5-4d3a-8e6f-2a5b9c1d3e47"
    with open(source, "rb") as f:
        raw = f.read()
    bom = raw.startswith(b"\xef\xbb\xbf")
    txt = raw.decode("utf-8-sig")
    nl = "\r\n" if "\r\n" in txt else "\n"
    type_xml = ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>%s</v8:Length>" % length,
                "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]
    block = attr_xml(name, synonym, type_xml, nl, 3)
    block = re.sub(r'<Attribute uuid="[^"]+">', '<Attribute uuid="%s">' % uuid, block, count=1)
    marker = "\t\t\t<TabularSection uuid=\"be5a03cb-a98b-4e33-8699-623c01a231a1\">"
    assert txt.count(marker) == 1, "marker"
    txt = txt.replace(marker, block + nl + marker)
    dst = os.path.join(out_dir, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "wb") as f:
        f.write((b"\xef\xbb\xbf" if bom else b"") + txt.encode("utf-8"))
    print("wrote", dst, len(txt))


if __name__ == "__main__":
    main()
