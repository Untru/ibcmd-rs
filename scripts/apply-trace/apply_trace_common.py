"""Helpers shared by trace_report.py and diff.py of the apply-trace kit.

Only the standard library is used, so the kit runs on a bare Python 3.9+.
"""

import gzip
import hashlib
import os
import re
import zlib

GUID_RE = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")

# Tables that store files as (FileName, PartNo) rows.
FILE_TABLES = ("Config", "ConfigSave", "ConfigCAS", "ConfigCASSave", "Params", "Files", "DepotFiles")
_FILE_TABLES_LOWER = {t.lower(): t for t in FILE_TABLES}


def canonical_table(name):
    """`dbo.[config]` -> `Config`; other names are only stripped of quoting."""
    n = name.strip().strip(";").strip()
    n = re.sub(r'[\[\]"]', "", n)
    if "." in n:
        n = n.split(".")[-1]
    return _FILE_TABLES_LOWER.get(n.lower(), n)


# ---------------------------------------------------------------------------
# TSV files written by the kit (backslash, tab, CR and LF are escaped)
# ---------------------------------------------------------------------------

_UNESC = {"\\": "\\", "t": "\t", "r": "\r", "n": "\n"}


def tsv_unescape(field):
    if "\\" not in field:
        return field
    out = []
    i = 0
    n = len(field)
    while i < n:
        c = field[i]
        if c == "\\" and i + 1 < n and field[i + 1] in _UNESC:
            out.append(_UNESC[field[i + 1]])
            i += 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def tsv_escape(value):
    if value is None:
        return ""
    s = str(value)
    if "\\" in s or "\t" in s or "\r" in s or "\n" in s:
        s = s.replace("\\", "\\\\").replace("\t", "\\t").replace("\r", "\\r").replace("\n", "\\n")
    return s


def read_tsv(path):
    """Yield one dict per data line of a kit TSV (first line = header); `.gz` files are read compressed."""
    opener = (lambda: gzip.open(path, "rt", encoding="utf-8", newline="")) if str(path).endswith(".gz") \
        else (lambda: open(path, "r", encoding="utf-8", newline=""))
    with opener() as fh:
        header = None
        for raw in fh:
            line = raw.rstrip("\n")
            if header is None:
                header = line.split("\t")
                continue
            if not line:
                continue
            parts = line.split("\t")
            if len(parts) < len(header):
                parts += [""] * (len(header) - len(parts))
            yield {h: tsv_unescape(v) for h, v in zip(header, parts)}


def write_tsv(path, header, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\t".join(header) + "\n")
        for row in rows:
            fh.write("\t".join(tsv_escape(row.get(h) if isinstance(row, dict) else row[i]) for i, h in enumerate(header)) + "\n")


# ---------------------------------------------------------------------------
# Row payloads
# ---------------------------------------------------------------------------

def sha256_hex(data):
    return hashlib.sha256(data).hexdigest()


def inflate_raw(data, max_bytes=256 * 1024 * 1024):
    """Raw-deflate inflate; None when `data` is not a raw-deflate stream."""
    if data is None or len(data) < 2:
        return None
    try:
        d = zlib.decompressobj(-15)
        out = d.decompress(data, max_bytes)
        # exactly one complete deflate stream, nothing before or after it
        if d.unconsumed_tail or not d.eof or d.unused_data:
            return None
        return out
    except zlib.error:
        return None


def content_kind(data):
    """empty | container | binary | v8text ({...} serialization) | text."""
    if not data:
        return "empty"
    off = 3 if data[:3] == b"\xef\xbb\xbf" else 0
    if data[:4] == b"\xff\xff\xff\x7f":
        return "container"
    probe = data[off:off + 4096]
    if not probe:
        return "empty"
    # text = well-formed UTF-8 without control characters (a multi-byte character
    # cut by the end of the probe window is not an error)
    cut_by_window = off + 4096 < len(data)
    try:
        probe.decode("utf-8")
    except UnicodeDecodeError as e:
        if not (cut_by_window and e.start >= len(probe) - 3):
            return "binary"
    controls = sum(1 for b in probe if b < 32 and b not in (9, 10, 13))
    if controls or probe.count(b"\x7f"):
        return "binary"
    return "v8text" if probe[:1] == b"{" else "text"


def decode_payload(stored):
    """Decode stored row bytes: inflate when possible, classify the result.

    Returns a dict: enc (deflate|raw), data (decoded bytes), kind, text (str for
    text kinds, else None).
    """
    inflated = inflate_raw(stored)
    if inflated is not None and len(inflated) > 0:
        enc, data = "deflate", inflated
    else:
        enc, data = "raw", stored
    kind = content_kind(data)
    text = None
    if kind in ("text", "v8text"):
        text = data.decode("utf-8-sig", errors="replace")
    return {"enc": enc, "data": data, "kind": kind, "text": text}


_BLOB_INDEX = {}


def _blob_index(store):
    """sha256 -> (pack file, offset, length) of a blob store (index.tsv), cached per process."""
    idx = _BLOB_INDEX.get(store)
    path = os.path.join(store, "index.tsv")
    if not os.path.isfile(path):
        return {}
    stamp = os.path.getmtime(path), os.path.getsize(path)
    if idx is not None and idx[0] == stamp:
        return idx[1]
    table = {}
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) >= 4 and len(parts[0]) == 64:
                table[parts[0]] = (parts[1], int(parts[2]), int(parts[3]))
    _BLOB_INDEX[store] = (stamp, table)
    return table


def load_blob(store, sha):
    """Stored bytes of a content-addressed blob, or None when the store does not hold it."""
    if not store or not sha:
        return None
    entry = _blob_index(store).get(sha)
    if entry is None:
        return None
    pack, off, length = entry
    with open(os.path.join(store, pack), "rb") as fh:
        fh.seek(off)
        raw = fh.read(length)
    return gzip.decompress(raw)


# ---------------------------------------------------------------------------
# Names of file rows
# ---------------------------------------------------------------------------

def name_shape(name):
    """`<guid>_dynupdate_<guid>.0` style shape of a Config/Params row name."""
    s = GUID_RE.sub("<guid>", name)
    # ConfigCAS names are content hashes
    s = re.sub(r"(?<![0-9A-Za-z])[0-9a-f]{64}(?![0-9A-Za-z])", "<sha256>", s)
    s = re.sub(r"(?<![0-9A-Za-z])[0-9a-f]{40}(?![0-9A-Za-z])", "<sha1>", s)
    s = re.sub(r"(?<![0-9A-Za-z])[0-9a-f]{32}(?![0-9A-Za-z])", "<md5>", s)
    s = re.sub(r"\.(\d+)(?=\.|$)", ".<n>", s)
    s = re.sub(r"(?<=-)(\d{2,})$", "<n>", s)
    return s


def one_line(text, limit=200):
    s = re.sub(r"\s+", " ", text).strip()
    return s if len(s) <= limit else s[: limit - 3] + "..."


# ---------------------------------------------------------------------------
# 1C {...} serialization: token-level diff and the `versions` codec
# ---------------------------------------------------------------------------

_TOKEN_RE = re.compile(r'"(?:[^"]|"")*"|[{}]|,|[^{},"]+')


def v8_tokens(text):
    """Tokens of a 1C `{...}` text: braces, commas, strings, atoms.

    Whitespace and line breaks between tokens are not tokens: the platform
    writes `{a,\\r\\n{b}}`, other writers `{a,{b}}`, and both are the same value.
    """
    out = []
    for t in _TOKEN_RE.findall(text):
        c = t[:1]
        if c == '"':
            # a line break inside a string is CRLF in the platform's files and LF elsewhere
            if "\r" in t:
                t = t.replace("\r\n", "\n")
        elif c not in ("{", "}", ","):
            t = t.strip()
            if not t:
                continue
            if t.startswith("#base64:"):
                # base64 wrapped into lines by the platform, on one line by other writers
                t = "#base64:" + "".join(t[8:].split())
        out.append(t)
    return out


def v8_lines(text, width=0):
    """One token per line (commas dropped), indented by brace depth."""
    lines = []
    depth = 0
    for t in v8_tokens(text):
        if t == ",":
            continue
        if t == "{":
            lines.append("  " * depth + "{")
            depth += 1
        elif t == "}":
            depth = max(0, depth - 1)
            lines.append("  " * depth + "}")
        else:
            lines.append("  " * depth + t)
    return lines


def parse_versions(text):
    """`versions` row: {1,<count>,"",<generation>, "<name>",<version>, ...}.

    Returns (header_tokens, {name: version}) or None when the text does not
    have that shape.
    """
    toks = [t for t in v8_tokens(text) if t not in (",",)]
    if len(toks) < 6 or toks[0] != "{":
        return None
    body = toks[1:-1] if toks[-1] == "}" else toks[1:]
    # body: 1, count, "", generation, then pairs
    if len(body) < 4:
        return None
    header = body[:4]
    pairs = body[4:]
    if len(pairs) % 2:
        return None
    mapping = {}
    for i in range(0, len(pairs), 2):
        mapping[pairs[i].strip('"')] = pairs[i + 1]
    return header, mapping


# ---------------------------------------------------------------------------
# 1C "format 15" file container (the inflated form of most module/form rows)
# ---------------------------------------------------------------------------

_BLOCK_HDR = 31   # \r\n + 8 hex + ' ' + 8 hex + ' ' + 8 hex + ' ' + \r\n


def _read_block(data, addr):
    hdr = data[addr:addr + _BLOCK_HDR]
    if len(hdr) < _BLOCK_HDR or hdr[:2] != b"\r\n" or hdr[10:11] != b" " or hdr[19:20] != b" " or hdr[28:29] != b" ":
        raise ValueError(f"bad block header at {addr}")
    doc_size = int(hdr[2:10], 16)
    page_size = int(hdr[11:19], 16)
    next_addr = int(hdr[20:28], 16)
    out = bytearray()
    pos = addr + _BLOCK_HDR
    remaining = doc_size
    while True:
        take = min(page_size, remaining)
        out += data[pos:pos + take]
        remaining -= take
        if remaining <= 0 or next_addr == 0x7FFFFFFF:
            break
        nh = data[next_addr:next_addr + _BLOCK_HDR]
        if len(nh) < _BLOCK_HDR:
            raise ValueError("bad continuation page")
        page_size = int(nh[11:19], 16)
        pos = next_addr + _BLOCK_HDR
        next_addr = int(nh[20:28], 16)
    return bytes(out)


def parse_v8_container(data):
    """[(name, header_bytes, body_bytes)] of a format-15 container, or None when it is not one."""
    try:
        if data[:4] != b"\xff\xff\xff\x7f" or len(data) < 16 + _BLOCK_HDR:
            return None
        toc = _read_block(data, 16)
        elements = []
        for i in range(0, len(toc) - 11, 12):
            hdr_addr = int.from_bytes(toc[i:i + 4], "little")
            body_addr = int.from_bytes(toc[i + 4:i + 8], "little")
            if hdr_addr == 0x7FFFFFFF:
                continue
            header = _read_block(data, hdr_addr)
            name = header[20:].decode("utf-16-le", errors="replace").rstrip("\x00")
            body = _read_block(data, body_addr) if body_addr != 0x7FFFFFFF else b""
            elements.append((name, header, body))
        return elements
    except (ValueError, IndexError):
        return None
