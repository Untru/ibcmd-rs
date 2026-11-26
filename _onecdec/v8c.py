"""Minimal read-only 1C v8 container reader for analysis (Format15 / Format16).

    python v8c.py <file> [--list] [--get NAME...] [--out DIR]

Format15: 16-byte file header, block header "\\r\\n%08x %08x %08x \\r\\n" (31 bytes),
TOC entries of three u32. Format16: a Format15 preamble (stub configuration for
old platforms) followed by the 64-bit primary container whose file header starts
with FF FF FF FF FF FF FF FF; block headers carry 16-hex-digit fields (55 bytes),
TOC entries are three u64, addresses are relative to the primary.
Entry bodies are raw DEFLATE (inflated when possible); a body that is itself a
container is returned as is.
"""
import os, struct, sys, zlib

W15 = bytes([0xFF, 0xFF, 0xFF, 0x7F])
W16 = bytes([0xFF] * 8)


class V8:
    def __init__(self, raw, primary=True):
        base = 0
        if primary and raw[:8] != W16:
            i = raw.find(W16, 0, 1 << 16)
            if i > 0 and raw[i + 20:i + 22] == b'\r\n':
                base = i
        self.base = base
        self.raw = raw[base:] if base else raw
        if self.raw[:8] == W16:
            self.wide, self.first = True, 20
        elif self.raw[:4] == W15:
            self.wide, self.first = False, 16
        else:
            raise ValueError('not a v8 container')
        self.end = 0xFFFFFFFFFFFFFFFF if self.wide else 0x7FFFFFFF

    def block(self, addr):
        raw, out, total = self.raw, bytearray(), None
        while True:
            if self.wide:
                h = raw[addr:addr + 55]
                if h[:2] != b'\r\n': raise ValueError('bad block header at %d' % addr)
                doc, page, nxt = int(h[2:18], 16), int(h[19:35], 16), int(h[36:52], 16)
                hl = 55
            else:
                h = raw[addr:addr + 31]
                if h[:2] != b'\r\n': raise ValueError('bad block header at %d' % addr)
                doc, page, nxt = int(h[2:10], 16), int(h[11:19], 16), int(h[20:28], 16)
                hl = 31
            if total is None: total = doc
            out += raw[addr + hl:addr + hl + page]
            if nxt in (self.end, 0x7FFFFFFF) or len(out) >= total: break
            addr = nxt
        return bytes(out[:total])

    def entries(self):
        toc = self.block(self.first)
        step, fmt = (24, '<QQQ') if self.wide else (12, '<III')
        for off in range(0, len(toc) - step + 1, step):
            ha, da, _ = struct.unpack_from(fmt, toc, off)
            hdr = self.block(ha)
            name = hdr[20:].decode('utf-16-le', 'replace').rstrip('\x00')
            yield name, da

    def body(self, da):
        blob = self.block(da)
        if blob[:4] == W15 or blob[:8] == W16:
            return blob
        try:
            return zlib.decompress(blob, -15)
        except zlib.error:
            return blob


def read(path):
    c = V8(open(path, 'rb').read())
    return {n: c.body(d) for n, d in c.entries()}


def main():
    a = sys.argv[1:]
    c = V8(open(a[0], 'rb').read())
    ents = dict(c.entries())
    if '--list' in a or len(a) == 1:
        for n in ents: print(n)
    if '--get' in a:
        i = a.index('--get'); names = [x for x in a[i + 1:] if not x.startswith('--')]
        out = a[a.index('--out') + 1] if '--out' in a else None
        for n in names:
            b = c.body(ents[n])
            if out:
                os.makedirs(out, exist_ok=True); open(os.path.join(out, n), 'wb').write(b)
            else:
                sys.stdout.buffer.write(b); sys.stdout.buffer.write(b'\n')


if __name__ == '__main__':
    main()


def _block15(data, page=None):
    page = max(page or 0, len(data), 512)
    return b'\r\n%08x %08x %08x \r\n' % (len(data), page, 0x7FFFFFFF) + data + b'\x00' * (page - len(data))


def write15(entries, storage_version=0, deflate_all=False):
    """{name: body bytes (plain)} -> Format15 container bytes; bodies are raw-deflated
    unless they already are a container. Analysis/spike use only."""
    import time as _t
    stamp = struct.pack('<Q', 0)
    out = bytearray(struct.pack('<IIII', 0x7FFFFFFF, 512, storage_version, 0))
    names = list(entries)
    toc_len = 12 * len(names)
    toc_at = len(out); out += _block15(b'\x00' * toc_len)
    addrs = []
    for n in names:
        body = entries[n]
        if deflate_all or not (body[:4] == W15 or body[:8] == W16):
            co = zlib.compressobj(9, zlib.DEFLATED, -15); body = co.compress(body) + co.flush()
        ha = len(out); out += _block15(stamp + stamp + b'\x00' * 4 + n.encode('utf-16-le') + b'\x00' * 4)
        da = len(out); out += _block15(body)
        addrs.append((ha, da))
    toc = b''.join(struct.pack('<III', ha, da, 0x7FFFFFFF) for ha, da in addrs)
    out[toc_at:toc_at + 31 + toc_len] = b'\r\n%08x %08x %08x \r\n' % (toc_len, max(toc_len, 512), 0x7FFFFFFF) + toc
    return bytes(out)
