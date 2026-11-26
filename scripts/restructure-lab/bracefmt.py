"""Parser and writer of the 1C brace text format ({a,b,{c,"str"}}).

Values: list -> python list, "str" -> Str (str subclass), everything else
(numbers, uuids, N, #, ...) -> Tok (str subclass, raw text).
"""


class Str(str):
    pass


class Tok(str):
    pass


def parse(text):
    if isinstance(text, bytes):
        text = text.decode("utf-8-sig")
    if text.startswith("﻿"):
        text = text[1:]
    pos = 0
    n = len(text)

    def skip_ws():
        nonlocal pos
        while pos < n and text[pos] in " \t\r\n":
            pos += 1

    def value():
        nonlocal pos
        skip_ws()
        c = text[pos]
        if c == "{":
            pos += 1
            items = []
            skip_ws()
            if text[pos] == "}":
                pos += 1
                return items
            while True:
                items.append(value())
                skip_ws()
                c2 = text[pos]
                pos += 1
                if c2 == ",":
                    continue
                if c2 == "}":
                    return items
                raise ValueError("bad char %r at %d" % (c2, pos - 1))
        if c == '"':
            pos += 1
            buf = []
            while True:
                j = text.index('"', pos)
                buf.append(text[pos:j])
                if j + 1 < n and text[j + 1] == '"':
                    buf.append('"')
                    pos = j + 2
                    continue
                pos = j + 1
                return Str("".join(buf))
        j = pos
        while j < n and text[j] not in ",}\r\n":
            j += 1
        tok = text[pos:j]
        pos = j
        return Tok(tok)

    v = value()
    skip_ws()
    if pos != n:
        raise ValueError("trailing data at %d of %d" % (pos, n))
    return v


def dumps(v, crlf=True):
    """The platform's layout: every nested list starts on a new line; a list whose last element is a
    list closes on a new line (so a list of scalars stays on one line)."""
    nl = "\r\n" if crlf else "\n"
    out = []

    def w(x):
        if isinstance(x, list):
            out.append("{")
            last_list = False
            for i, y in enumerate(x):
                if i:
                    out.append(",")
                if isinstance(y, list):
                    out.append(nl)
                    w(y)
                    last_list = True
                else:
                    w(y)
                    last_list = False
            if last_list:
                out.append(nl)
            out.append("}")
        elif isinstance(x, Str):
            out.append('"' + x.replace('"', '""') + '"')
        else:
            out.append(str(x))

    w(v)
    return "".join(out)
