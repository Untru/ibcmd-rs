"""Derive the closed-module fixture from `test_extension`: its common module
replaced by a module without source text -- an inner container of `image` and
`info` under storage revision 0, the shape a delivery without source keeps
(MONITOR-TRIAL-2.cf: 204 such modules; 8.3.27.2214 dumps each as `<Module>.bin`,
the inflated entry byte for byte). The image is a stand-in, not compiled code.

    python make_closed_module_fixture.py

Output: tests/fixtures/external/closed_module/{input.cfe, Module.bin}
"""
import base64, hashlib, os, re, struct, sys, zlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8c

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = os.path.join(os.path.dirname(HERE), 'tests', 'fixtures', 'external')
MODULE = '554f39c6-e029-4ee0-8535-8c15291a6a3f.0'


def inner_container(elements, revision):
    """Format15 container with raw (not deflated) element bodies."""
    stamp = b'\x00' * 8
    out = bytearray(struct.pack('<IIII', 0x7FFFFFFF, 512, revision, 0))
    toc_at = len(out)
    out += v8c._block15(b'\x00' * (12 * len(elements)))
    addresses = []
    for name, body in elements:
        header_at = len(out)
        out += v8c._block15(stamp + stamp + b'\x00' * 4 + name.encode('utf-16-le') + b'\x00' * 4)
        body_at = len(out)
        out += v8c._block15(body)
        addresses.append((header_at, body_at))
    toc = b''.join(struct.pack('<III', h, d, 0x7FFFFFFF) for h, d in addresses)
    page = max(len(toc), 512)
    out[toc_at:toc_at + 31 + len(toc)] = b'\r\n%08x %08x %08x \r\n' % (len(toc), page, 0x7FFFFFFF) + toc
    return bytes(out)


def packed_bodies(container):
    archive = v8c.V8(container)
    return {name: archive.block(address) for name, address in archive.entries()}


def refresh_manifest(entries, container):
    # configinfo hashes the packed rows. write15 recompresses every row, so
    # even unchanged inflated bodies need their actual output digest refreshed.
    packed = packed_bodies(container)
    seen = set()

    def digest(match):
        name = match[1].decode('utf-8')
        assert name in packed and name != 'configinfo' and name not in seen
        seen.add(name)
        value = base64.b64encode(hashlib.sha1(packed[name]).digest())
        return b'"' + match[1] + b'",' + value

    updated = re.sub(rb'"([^"]+)",([A-Za-z0-9+/]{27}=)', digest, entries['configinfo'])
    assert seen == set(entries) - {'configinfo'}, 'manifest must cover every data row once'
    return updated


def main():
    source = os.path.join(FIXTURES, 'test_extension', 'input.cfe')
    raw = open(source, 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(source)
    image = '﻿{1,\r\n{"Cmd",1,0,\r\n{0}\r\n},\r\n{"Const",0},\r\n{"Var",0},\r\n{"Proc",0}\r\n}'.encode('utf-8')
    info = '﻿{3,2,0,"",0}'.encode('utf-8')
    module = inner_container([('image', image), ('info', info)], 0)
    entries[MODULE] = module
    dst = os.path.join(FIXTURES, 'closed_module')
    os.makedirs(dst, exist_ok=True)
    # write15 keeps a body that is itself a container as is (not deflated);
    # the export inflates entries, so deflate the module entry ourselves.
    container = v8c.write15(entries, storage_version, deflate_all=True)
    entries['configinfo'] = refresh_manifest(entries, container)
    container = v8c.write15(entries, storage_version, deflate_all=True)
    assert refresh_manifest(entries, container) == entries['configinfo']
    open(os.path.join(dst, 'input.cfe'), 'wb').write(container)
    open(os.path.join(dst, 'Module.bin'), 'wb').write(module)
    check = v8c.V8(container)
    stored = dict(check.entries())[MODULE]
    assert zlib.decompress(check.block(stored), -15) == module
    print('ok', len(module), 'bytes')


if __name__ == '__main__':
    main()
