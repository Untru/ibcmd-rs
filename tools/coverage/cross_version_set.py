#!/usr/bin/env python3
"""Prove an unchanged public native CF on a second platform build.

Importing XML instead would rewrite the stored form layout and hide the
cross-version record whose export is being tested.
"""
import argparse
import base64
import datetime
import json
import os
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from native_round_trip import export, first_error, inputs_digest, run_ibcmd, sha256, tree_files


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--source', type=pathlib.Path, required=True)
    parser.add_argument('--set', type=pathlib.Path, required=True)
    parser.add_argument('--work', type=pathlib.Path, required=True)
    parser.add_argument('--ibcmd', required=True)
    parser.add_argument('--platform', required=True)
    parser.add_argument('--xml-version', required=True)
    args = parser.parse_args()
    if args.work.exists() or args.set.exists():
        raise SystemExit('use new work and set directories')
    args.work.mkdir(parents=True)
    source_manifest = args.source / 'manifest.json'
    original = json.loads(source_manifest.read_text(encoding='utf-8'))
    encoded = (args.source / 'configuration.cf.b64').read_bytes()
    raw = base64.b64decode(encoded, validate=False)
    if sha256(raw) != original['configuration_cf']['decoded_sha256']:
        raise SystemExit('source CF digest does not match its manifest')
    cf = args.work / 'original.cf'
    cf.write_bytes(raw)
    for number in [1, 2]:
        database = args.work / f'ib{number}'
        ok, output = run_ibcmd(args, args.ibcmd, args.work, 'infobase', 'create',
                               f'--db-path={database}', '--create-database',
                               '--locale=ru_RU', f'--load={cf}', '--apply', '--force')
        if not ok:
            raise SystemExit('native cross-version load: ' + first_error(output))
        export(args, args.work, database, args.work / f'round{number}')
    first, second = tree_files(args.work / 'round1'), tree_files(args.work / 'round2')
    if first != second:
        raise SystemExit('native exports of the unchanged input CF differ')
    variants = []
    claimed = set()
    for variant in original['variants']:
        if variant['status'] != 'platform_proven':
            continue
        v = dict(variant)
        owned = {f['path'] for f in variant['files']}
        v['files'] = [{'path': path, 'size': len(data), 'sha256': sha256(data)}
                      for path, data in second.items() if path in owned]
        claimed.update(f['path'] for f in v['files'])
        variants.append(v)
    # New platform sidecars belong to their existing owner, or the root.
    for path in sorted(second.keys() - claimed):
        parts = path.split('/')
        owner = next((v for v in variants if len(parts) > 1 and parts[0] == original['collection']
                      and v['name'] == parts[1].removesuffix('.xml')), variants[0])
        owner['files'].append({'path': path, 'size': len(second[path]), 'sha256': sha256(second[path])})
    inputs = args.set / 'input'
    inputs.mkdir(parents=True)
    dependency = {'set': original['set'], 'path': pathlib.Path(os.path.relpath(args.source, args.set.parent)).as_posix(),
                  'manifest_sha256': sha256(source_manifest.read_bytes())}
    (inputs / 'source.json').write_text(json.dumps(dependency, indent=2) + '\n', encoding='utf-8')
    for path, data in second.items():
        destination = args.set / 'native' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    (args.set / 'configuration.cf.b64').write_bytes(encoded)
    (args.set / 'baseline.json').write_text('{"equal": []}\n', encoding='utf-8')
    manifest = {'schema_version': 1, 'kind': 'cross_version',
                'set': f'{args.platform}-xml-{args.xml_version}-{args.set.name}',
                'collection': original['collection'],
                'evidence': {'status': 'platform_proven', 'created_on': datetime.date.today().isoformat(),
                             'platform_version': args.platform, 'source_version': args.xml_version,
                             'ibcmd_sha256': sha256(pathlib.Path(args.ibcmd).read_bytes()),
                             'database_kind': 'isolated_file_infobase',
                             'rounds': 'unchanged source native CF -> load/apply/export twice; identical outside ConfigDumpInfo.xml',
                             'source_platform': original['evidence']['platform_version']},
                'sources': [dependency], 'inputs_sha256': inputs_digest(args.set),
                'configuration_cf': {'path': 'configuration.cf.b64', 'decoded_size': len(raw),
                                     'decoded_sha256': sha256(raw), 'encoded_sha256': sha256(encoded)},
                'variants': variants}
    (args.set / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'{len(variants)} cross-version variants proven')


if __name__ == '__main__':
    main()
