"""A minimal reader/writer of the 1C V8 container (.cf/.cfe/.epf): enough to
repack a file with one property changed and see what the platform accepts.

    python v8container.py <in> <out> [--zero-stamps] [--compact] [--sort]

Layout: a 16-byte header (next free page 0x7fffffff, page size, storage
version, reserved), then blocks `\\r\\n<data len> <block len> <next> \\r\\n<data>`
(eight hex digits each). The first block holds the table of contents:
(header address, data address, 0x7fffffff) per element. An element header is
two times, four zero bytes, the name in UTF-16LE, four zero bytes.
"""
import struct, sys

END = 0x7fffffff


def read_block(buf, addr):
    """The whole data of the block chain starting at `addr`."""
    out = b''
    total = None
    while addr != END:
        head = buf[addr:addr + 31]
        assert head[:2] == b'\r\n' and head[29:31] == b'\r\n', hex(addr)
        data_len, block_len, nxt = (int(head[i:i + 8], 16) for i in (2, 11, 20))
        if total is None:
            total = data_len
        out += buf[addr + 31:addr + 31 + block_len]
        addr = nxt
    return out[:total]


def read(path):
    buf = open(path, 'rb').read()
    free, page, version, reserved = struct.unpack('<4I', buf[:16])
    toc = read_block(buf, 16)
    elements = []
    for i in range(0, len(toc) - len(toc) % 12, 12):
        h, d, _ = struct.unpack('<3I', toc[i:i + 12])
        header = read_block(buf, h)
        data = read_block(buf, d) if d != END else None
        elements.append((header, data))
    return (page, version, reserved), elements


def block(data, capacity):
    capacity = max(capacity, len(data))
    return b'\r\n%08x %08x %08x \r\n' % (len(data), capacity, END) + data + b'\0' * (capacity - len(data))


def write(path, meta, elements, compact=False):
    page, version, reserved = meta
    out = bytearray(struct.pack('<4I', END, page, version, reserved))
    toc_cap = len(elements) * 12 if compact else max(page, len(elements) * 12)
    toc_at = len(out)
    out += block(b'\0' * (len(elements) * 12), toc_cap)
    toc = bytearray()
    for header, data in elements:
        h = len(out)
        out += block(header, len(header))
        if data is None:
            d = END
        else:
            d = len(out)
            out += block(data, len(data) if compact else max(page, len(data)))
        toc += struct.pack('<3I', h, d, END)
    out[toc_at:toc_at + 31 + len(toc)] = block(bytes(toc), toc_cap)[:31 + len(toc)]
    open(path, 'wb').write(out)


def name_of(header):
    return header[20:-4].decode('utf-16-le')


def main():
    src, dst = sys.argv[1], sys.argv[2]
    meta, elements = read(src)
    if '--zero-stamps' in sys.argv:
        elements = [(b'\0' * 20 + h[20:], d) for h, d in elements]
    if '--sort' in sys.argv:
        elements.sort(key=lambda e: name_of(e[0]))
    write(dst, meta, elements, compact='--compact' in sys.argv)
    print(len(elements), 'elements', meta)


if __name__ == '__main__':
    main()
