"""Registry (1a621f0f) records inserted between two snapshots."""
import sys

from rowdiff import ROWS, get, load, row_full


def members(text, pos):
    """members of the list starting at text[pos] == '{' -> (spans, end)"""
    assert text[pos] == "{"
    pos += 1
    out = []
    while True:
        while text[pos] in " \t\r\n":
            pos += 1
        start = pos
        if text[pos] == '"':
            pos += 1
            while True:
                if text[pos] == '"':
                    if text[pos + 1] == '"':
                        pos += 2
                        continue
                    pos += 1
                    break
                pos += 1
        elif text[pos] == "{":
            depth = 0
            while True:
                c = text[pos]
                if c == '"':
                    pos += 1
                    while not (text[pos] == '"' and text[pos + 1] != '"'):
                        pos += 2 if text[pos] == '"' else 1
                    pos += 1
                    continue
                if c == "{":
                    depth += 1
                elif c == "}":
                    depth -= 1
                    if depth == 0:
                        pos += 1
                        break
                pos += 1
        else:
            while text[pos] not in ",} \t\r\n":
                pos += 1
        out.append((start, pos))
        while text[pos] in " \t\r\n":
            pos += 1
        if text[pos] == ",":
            pos += 1
            continue
        assert text[pos] == "}"
        return out, pos + 1


def parse(raw):
    text = raw.decode("utf-8-sig")
    top, _ = members(text, 0)
    classes, _ = members(text, top[1][0])
    recs, _ = members(text, top[2][0])
    classes = [text[a:b] for a, b in classes[1:]]
    out = []
    for i in range(1, len(recs), 7):
        g = recs[i:i + 7]
        out.append(dict(
            uuid=text[g[0][0]:g[0][1]],
            parent=text[g[1][0]:g[1][1]],
            kind=int(text[g[2][0]:g[2][1]]),
            name=text[g[3][0]:g[3][1]],
            syn=text[g[4][0]:g[4][1]],
            flags=(text[g[5][0]:g[5][1]], text[g[6][0]:g[6][1]]),
        ))
    return classes, out


def lcs_inserts(a, b):
    """records of b not in a, in order of b, each with the uuid of the b record before it"""
    ia = {r["uuid"]: r for r in a}
    ib = {r["uuid"] for r in b}
    removed = [r for r in a if r["uuid"] not in ib]
    ins = []
    prev = None
    for r in b:
        if r["uuid"] not in ia:
            ins.append((prev, r))
        prev = r["uuid"]
    return ins, removed


if __name__ == "__main__":
    a = load(*sys.argv[1].split(":"))
    b = load(*sys.argv[2].split(":"))
    ca, ra = parse(get(a, "Params", row_full("1a621f0f", a)))
    cb, rb = parse(get(b, "Params", row_full("1a621f0f", b)))
    print("classes same:", ca == cb, len(ra), len(rb))
    ins, removed = lcs_inserts(ra, rb)
    print("removed", len(removed))
    names = {r["uuid"]: r["name"] for r in rb}
    for prev, r in ins:
        print("after", names.get(prev), "|", r["uuid"], "parent", names.get(r["parent"], r["parent"]), "kind", r["kind"], r["name"], r["flags"], r["syn"].replace("\r\n", " ")[:80])
