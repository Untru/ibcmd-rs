"""XDTO lines inserted between two snapshots (each with the object type it stands in)."""
import base64
import difflib
import sys

from rowdiff import get, load, row_full


def model_xml(raw):
    text = raw.decode("utf-8-sig")
    marker = "{#base64:"
    a = text.index(marker) + len(marker)
    b = text.rindex("}")
    body = text[a:b].replace("\r", "").replace("\n", "")
    xml = base64.b64decode(body).decode("utf-8-sig")
    return xml


if __name__ == "__main__":
    a = load(*sys.argv[1].split(":"))
    b = load(*sys.argv[2].split(":"))
    xa = model_xml(get(a, "Params", row_full("ea13a2c9", a))).split("\r\n")
    xb = model_xml(get(b, "Params", row_full("ea13a2c9", b))).split("\r\n")
    print("lines", len(xa), len(xb))
    sm = difflib.SequenceMatcher(None, xa, xb, autojunk=False)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            continue
        # object type context: nearest <objectType before j1
        ctx = ""
        for k in range(j1 - 1, -1, -1):
            if "<objectType" in xb[k]:
                ctx = xb[k].strip()
                break
        print(tag, "at", j1, "ctx", ctx)
        for line in xa[i1:i2]:
            print("   -", line)
        for line in xb[j1:j2]:
            print("   +", line)
