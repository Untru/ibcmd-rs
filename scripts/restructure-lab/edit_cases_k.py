"""Case k: data conversion on Catalog.КлючевыеОперации (attributes added in case h):
   ДемоСтрокаНеогр  String(0)      -> String(5)       (narrowing, rows hold longer text)
   ДемоЧисло        Number(10,0)   -> Number(5,0)     (narrowing, rows hold 123456)
   ДемоБулево       Boolean        -> String(10)      (type change)
"""
import os
import re

import edit_tree as E


def case_k():
    p = os.path.join("Catalogs", "КлючевыеОперации.xml")
    E.save_orig("k", p)
    txt, bom = E.read(p)

    def edit(name, old, new):
        nonlocal txt
        m = re.search(r'<Attribute uuid="[^"]+">\s*<Properties>\s*<Name>%s</Name>.*?</Attribute>' % name, txt, re.S)
        assert m, name
        blk = m.group(0)
        assert old in blk, (name, old)
        txt = txt[:m.start()] + blk.replace(old, new, 1) + txt[m.end():]

    edit("ДемоСтрокаНеогр", "<v8:Length>0</v8:Length>", "<v8:Length>5</v8:Length>")
    edit("ДемоЧисло", "<v8:Digits>10</v8:Digits>", "<v8:Digits>5</v8:Digits>")
    edit("ДемоБулево", "<v8:Type>xs:boolean</v8:Type>",
         "<v8:Type>xs:string</v8:Type>\n\t\t\t\t\t\t<v8:StringQualifiers>\n\t\t\t\t\t\t\t<v8:Length>10</v8:Length>\n"
         "\t\t\t\t\t\t\t<v8:AllowedLength>Variable</v8:AllowedLength>\n\t\t\t\t\t\t</v8:StringQualifiers>")
    E.write(p, txt, bom)
    E.save_new("k", p)
    print("case k: narrowing string, narrowing number, boolean -> string")
