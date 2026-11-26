"""DBNames decoding and the uuid -> metadata name map (ConfigDumpInfo.xml).

DBNames blob (Params.FileName='DBNames') is raw-deflated 1C brace text:
   {<max number>,{<count>,{<uuid>,"<kind>",<number>},...}}
"""
import re
import zlib

NIL = "00000000-0000-0000-0000-000000000000"


def inflate_dbnames(blob):
    return zlib.decompress(blob, -15)


def parse_dbnames(text):
    """-> (max_number, [(uuid, kind, number)], raw header info)"""
    if isinstance(text, bytes):
        text = text.decode("utf-8-sig")
    m = re.match(r"\s*\{(\d+),\s*\{(\d+),", text)
    maxn, count = int(m.group(1)), int(m.group(2))
    ents = [(a, b, int(c)) for a, b, c in
            re.findall(r'\{([0-9a-f-]{36}),"([^"]*)",(\d+)\}', text)]
    return maxn, count, ents


def dumpinfo_names(path):
    """uuid -> qualified name for every Metadata element in ConfigDumpInfo.xml."""
    out = {}
    with open(path, encoding="utf-8-sig") as f:
        for m in re.finditer(r'<Metadata name="([^"]+)" id="([^"]+)"', f.read()):
            out[m.group(2)] = m.group(1)
    return out
