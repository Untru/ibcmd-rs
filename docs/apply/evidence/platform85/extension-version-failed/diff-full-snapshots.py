"""Read-only complete physical snapshot delta; no normalization/omissions."""
import argparse
import collections
import hashlib
import json
import pathlib

from compare_full_snapshots import AUX, STORAGE, load_checked


def verify_ranges(path, obj):
    root = pathlib.Path(path).parent
    for table in sorted(STORAGE):
        info = obj['storage'][table]
        if len(info['rows']) > 20000 or info['bytes'] > 512 * 1024 * 1024:
            raise ValueError('storage budget exceeded')
        cursor = 0
        groups = {}
        with (root / info['pack']).open('rb') as stream:
            for row in info['rows']:
                if row['offset'] != cursor or not 0 <= row['byte_len'] <= 64 * 1024 * 1024:
                    raise ValueError('noncontiguous row range')
                remaining = row['byte_len']
                digest = hashlib.sha256()
                while remaining:
                    block = stream.read(min(65536, remaining))
                    if not block:
                        raise ValueError('short row range')
                    digest.update(block)
                    remaining -= len(block)
                if digest.hexdigest() != row['sha256']:
                    raise ValueError('row range digest differs')
                cursor += row['byte_len']
                groups.setdefault(row['name'], []).append(row)
        if cursor != info['bytes']:
            raise ValueError('row range total differs')
        for rows in groups.values():
            parts = sorted(rows, key=lambda row: row['part'])
            if [r['part'] for r in parts] != list(range(len(parts))):
                raise ValueError('multipart parts not contiguous')
            # DataSize is the entire logical blob size on each physical part.
            size = sum(r['byte_len'] for r in parts)
            if any(r['data_size'] != size for r in parts):
                raise ValueError('multipart DataSize differs from group total')


def keyed(rows):
    result = {}
    for row in rows:
        key = (row['name'], row['part'])
        if key in result:
            raise ValueError('duplicate physical row key')
        # Offset belongs to this capture's pack, not to the SQL row header.
        result[key] = {k: v for k, v in row.items() if k != 'offset'}
    return result


def delta(a, b):
    if a['database'] != b['database'] or set(a['storage']) != STORAGE or set(b['storage']) != STORAGE or set(a['auxiliary']) != AUX or set(b['auxiliary']) != AUX:
        raise ValueError('database/table scope differs')
    report = {'database': a['database'], 'before_tag': a['tag'], 'after_tag': b['tag'], 'normalization': 'none; pack offsets are capture locations and excluded only from SQL row-header delta', 'storage': {}, 'auxiliary': {}}
    for table in sorted(STORAGE):
        x, y = keyed(a['storage'][table]['rows']), keyed(b['storage'][table]['rows'])
        common = x.keys() & y.keys()
        changed = []
        for key in sorted(common):
            if x[key] != y[key]:
                changed.append({'name': key[0], 'part': key[1], 'fields': [k for k in x[key] if x[key][k] != y[key][k]], 'before': x[key], 'after': y[key]})
        report['storage'][table] = {'before_rows': len(x), 'after_rows': len(y), 'unchanged_rows': sum(x[k] == y[k] for k in common), 'added': [y[k] for k in sorted(y.keys() - x.keys())], 'removed': [x[k] for k in sorted(x.keys() - y.keys())], 'changed': changed}
    for table in sorted(AUX):
        x, y = a['auxiliary'][table], b['auxiliary'][table]
        if x['columns'] != y['columns']:
            raise ValueError('auxiliary columns changed')
        def multiset(rows):
            return collections.Counter(json.dumps(r, ensure_ascii=False, sort_keys=True, separators=(',', ':')) for r in rows)
        left, right = multiset(x['rows']), multiset(y['rows'])
        report['auxiliary'][table] = {'columns': x['columns'], 'before_rows': len(x['rows']), 'after_rows': len(y['rows']), 'added': [json.loads(r) for r, n in sorted((right - left).items()) for _ in range(n)], 'removed': [json.loads(r) for r, n in sorted((left - right).items()) for _ in range(n)]}
    return report


def self_test():
    import copy
    row = {'name': 'x', 'part': 0, 'creation': 'c', 'modified': 'm', 'attributes': 0, 'data_size': 1, 'byte_len': 1, 'sha256': 'a', 'offset': 0}
    a = {'database': 'owned', 'tag': 'before', 'storage': {t: {'rows': [row.copy()]} for t in STORAGE}, 'auxiliary': {t: {'columns': ['id'], 'rows': [[1], [2]]} for t in AUX}}
    b = copy.deepcopy(a)
    b['tag'] = 'after'
    b['storage']['Config']['rows'][0]['offset'] = 10
    assert not delta(a, b)['storage']['Config']['changed']
    count = 1
    for field in ('creation', 'modified', 'attributes', 'data_size', 'byte_len', 'sha256'):
        c = copy.deepcopy(b)
        c['storage']['Config']['rows'][0][field] = 'different'
        assert delta(a, c)['storage']['Config']['changed'][0]['fields'] == [field]
        count += 1
    b['storage']['ConfigSave']['rows'] = []
    assert delta(a, b)['storage']['ConfigSave']['removed'] == [{k: v for k, v in row.items() if k != 'offset'}]
    count += 1
    b['auxiliary']['_ExtensionsInfo']['rows'].reverse()
    assert not delta(a, b)['auxiliary']['_ExtensionsInfo']['added']
    count += 1
    b['auxiliary']['_ExtensionsInfo']['rows'].append([1])
    assert delta(a, b)['auxiliary']['_ExtensionsInfo']['added'] == [[1]]
    count += 1
    print(f'PASS complete physical delta {count} cases; no SQL/process/write')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--before')
    parser.add_argument('--after')
    parser.add_argument('--output')
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        before, after = load_checked(args.before), load_checked(args.after)
        verify_ranges(args.before, before)
        verify_ranges(args.after, after)
        result = delta(before, after)
        with pathlib.Path(args.output).open('x', encoding='utf8') as f:
            json.dump(result, f, ensure_ascii=False, indent=2)
            f.write('\n')
        print(json.dumps({t: {'added': len(v['added']), 'removed': len(v['removed']), 'changed': len(v['changed']), 'unchanged': v['unchanged_rows']} for t, v in result['storage'].items()}))
        print(json.dumps({t: {'added': len(v['added']), 'removed': len(v['removed'])} for t, v in result['auxiliary'].items()}))
