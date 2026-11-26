#!/usr/bin/env python3
"""Compare the `deleted` row of two stages: format byte for byte, names as sets, and the order.

usage: deleted_cmp.py <native.txt|snap:<db>/<label>> <ours...>
A source is either a file with the inflated text, or `snap:<db>/<label>` (the ConfigSave `deleted` blob of a ddl-kit snapshot).
"""
import json
import re
import sys
import zlib

sys.stdout.reconfigure(encoding='utf-8')
KIT = 'F:/ibcmd/lab/04/apply/ddlkit'


def load(source):
    if source.startswith('snap:'):
        db, label = source[5:].split('/')
        svc = json.load(open('%s/snap/%s/%s/svc.json' % (KIT, db, label), encoding='utf-8'))
        for entry in svc['ConfigSave']:
            if entry['name'] == 'deleted':
                raw = open('%s/blobs/%s' % (KIT, entry['sha']), 'rb').read()
                return zlib.decompress(raw, -15), raw
        raise SystemExit('no deleted row in ' + source)
    raw = open(source, 'rb').read()
    return raw, None


def analyse(label, text):
    body = text.decode('utf-8')
    shape = re.fullmatch(r'\ufeff(\d+)((?:,"[^"]+",[01])*)', body)
    names = re.findall(r'"([^"]+)",([01])', body)
    return {
        'label': label,
        'bytes': len(text),
        'bom': text.startswith(b'\xef\xbb\xbf'),
        'format_ok': bool(shape) and int(shape.group(1)) == len(names),
        'count': len(names),
        'names': names,
        'ends_with_newline': text.endswith(b'\n'),
    }


sources = sys.argv[1:]
infos = []
for source in sources:
    text, raw = load(source)
    info = analyse(source, text)
    info['deflated'] = len(raw) if raw else None
    infos.append(info)
    print('%-60s bytes %d, deflated %s, BOM %s, count %d, format ok %s, trailing newline %s' % (
        source, info['bytes'], info['deflated'], info['bom'], info['count'], info['format_ok'], info['ends_with_newline']))
base = infos[0]
for other in infos[1:]:
    same_set = sorted(base['names']) == sorted(other['names'])
    same_order = base['names'] == other['names']
    print('%s against %s: same set of (name, flag) %s, same order %s, same length %s' % (
        other['label'], base['label'], same_set, same_order, base['bytes'] == other['bytes']))
