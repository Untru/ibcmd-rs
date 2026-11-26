"""Validate exact native witnesses and distinguish known symptoms from regressions.

The corpus owns native inputs. This consumer-owned ledger describes converter
diagnostics; a missing widget/file never substitutes for one missing property.
"""
import argparse
import base64
import collections
import hashlib
import io
import re
import json
import os
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET



XSI_TYPE = '{http://www.w3.org/2001/XMLSchema-instance}type'
# XML 1.0 NCName, excluding the colon reserved for the QName prefix separator.
NAME_START = (
    r'A-Z_a-z\u00C0-\u00D6\u00D8-\u00F6\u00F8-\u02FF'
    r'\u0370-\u037D\u037F-\u1FFF\u200C-\u200D\u2070-\u218F'
    r'\u2C00-\u2FEF\u3001-\uD7FF\uF900-\uFDCF\uFDF0-\uFFFD'
    r'\U00010000-\U000EFFFF'
)
NCNAME = re.compile('[' + NAME_START + '][' + NAME_START
                    + r'\-.0-9\u00B7\u0300-\u036F\u203F-\u2040]*\Z')


def parse_xml(data):
    """Retain xsi:type meaning before ElementTree discards prefix bindings."""
    parser = ET.iterparse(io.BytesIO(data), events=('start-ns', 'start', 'end'))
    scopes, pending = [], {}
    for event, value in parser:
        if event == 'start-ns':
            prefix, uri = value
            pending[prefix or ''] = uri
        elif event == 'start':
            scope = dict(scopes[-1]) if scopes else {'xml': 'http://www.w3.org/XML/1998/namespace'}
            scope.update(pending)
            pending.clear()
            scopes.append(scope)
            lexical = value.get(XSI_TYPE)
            if lexical is not None:
                parts = lexical.strip(' \t\r\n').split(':')
                if len(parts) not in (1, 2) or any(NCNAME.fullmatch(part) is None for part in parts):
                    raise ValueError('invalid xsi:type QName')
                if len(parts) == 2:
                    prefix, local = parts
                    if not scope.get(prefix):
                        raise ValueError('undeclared xsi:type namespace prefix')
                    uri = scope[prefix]
                else:
                    local, uri = parts[0], scope.get('', '')
                value.set(XSI_TYPE, '{' + uri + '}' + local if uri else local)
        else:
            scopes.pop()
    return parser.root


def canonical(element):
    if element is None:
        return None
    value = element.text or ''
    # Leaf whitespace belongs to the value (for example an input mask).
    # Indentation preceding child elements is XML formatting only.
    if len(element) and not value.strip():
        value = ''
    return [element.tag, sorted(element.attrib.items()), value,
            [canonical(child) for child in element]]


def condition(root, case):
    anchor = case['anchor']
    if 'collection' in case and (case['collection'] is not True or anchor.get('root') is not True):
        raise ValueError('collection witness requires an exact root context')
    if anchor.get('root') is True:
        if set(anchor) != {'tag', 'root', 'context'}:
            raise ValueError('root witness requires one exact document context')
        for node in root.iter():
            if node.tail and node.tail.strip(' \t\r\n'):
                raise ValueError('document contains unexpected text between elements')
            # The shared scalar canonicalizer treats Unicode whitespace as
            # indentation. Document guards allow only XML indentation here.
            if len(node) and node.text and not node.text.strip() and node.text.strip(' \t\r\n'):
                raise ValueError('document contains non-XML indentation text')
        context = canonical(root)
        context[3] = [item for item in context[3] if item[0] != case['property']]
        if root.tag != anchor['tag'] or json.loads(json.dumps(context)) != anchor['context']:
            raise ValueError(f"{case['id']}: intact document context differs")
        anchors = [root]
    else:
        child = anchor.get('child')
        if child is not None:
            values = (case['native'], case['known_symptom'])
            if any(not isinstance(value, list) or len(value) != 4
                   or value[0] != case['property'] or value[1] or value[3]
                   or not isinstance(value[2], str) or not value[2] for value in values):
                raise ValueError('child identity requires two exact scalar text conditions')
            aliases = child['texts']
            if child['tag'] != case['property'] or len(aliases) != 2 or set(aliases) != {value[2] for value in values}:
                raise ValueError('child identity must contain exactly the native and known names')
        elif 'name' not in anchor:
            raise ValueError('witness anchor requires a name or child identity')
        anchors = []
        for node in root.iter():
            if node.tag != anchor['tag']:
                continue
            if 'name' in anchor and node.get('name') != anchor['name']:
                continue
            if any(node.get(key) != value for key, value in anchor.get('attributes', {}).items()):
                continue
            if child is not None and not any(item.tag == child['tag'] and item.text in aliases for item in node):
                continue
            anchors.append(node)
    if len(anchors) != 1:
        raise ValueError(f"{case['id']}: expected one intact widget, got {len(anchors)}")
    children = [node for node in anchors[0] if node.tag == case['property']]
    if case.get('collection') is True:
        return json.loads(json.dumps([canonical(child) for child in children]))
    if len(children) > 1:
        raise ValueError(f"{case['id']}: repeated property")
    # JSON uses arrays for attribute pairs; normalize tuples before comparing.
    return json.loads(json.dumps(canonical(children[0] if children else None)))


def classify(native, ours, case):
    if condition(native, case) != case['native']:
        raise ValueError(f"{case['id']}: native no longer retains the witness")
    observed = condition(ours, case)
    if observed == case['native']:
        return 'equal'
    if observed == case['known_symptom']:
        return 'known-symptom'
    raise ValueError(f"{case['id']}: an unrelated mismatch replaced the known symptom")


def validate_cases(corpus, ledger):
    if ledger['schema_version'] != 1 or not ledger['cases']:
        raise ValueError('invalid or empty observed-problem ledger')
    ids = set()
    root = corpus.resolve()
    for case in ledger['cases']:
        if case['id'] in ids or case['native'] == case['known_symptom']:
            raise ValueError('duplicate case or indistinguishable symptom')
        ids.add(case['id'])
        source = (corpus / 'coverage' / case['set']).resolve()
        if not source.is_relative_to(root):
            raise ValueError('case escapes the pinned corpus')
        manifest = json.loads((source / 'manifest.json').read_text(encoding='utf-8'))
        if manifest.get('evidence', {}).get('status') != 'platform_proven':
            raise ValueError(f"{case['id']}: source lacks native proof")
        files = [item for variant in manifest['variants']
                 if variant['status'] == 'platform_proven' and variant['name'] == case['variant']
                 for item in variant['files'] if item['path'] == case['file']]
        target = (source / 'native' / case['file']).resolve()
        if not target.is_relative_to(source / 'native') or len(files) != 1:
            raise ValueError(f"{case['id']}: witness is not owned by the proven variant")
        data = target.read_bytes()
        if hashlib.sha256(data).hexdigest() != files[0]['sha256']:
            raise ValueError(f"{case['id']}: native file hash differs")
        if condition(parse_xml(data), case) != case['native']:
            raise ValueError(f"{case['id']}: native condition is not present")


def run(corpus, ledger, binary):
    validate_cases(corpus, ledger)
    counts = collections.Counter()
    grouped = collections.defaultdict(list)
    for case in ledger['cases']:
        grouped[case['set']].append(case)
    with tempfile.TemporaryDirectory(prefix='ibcmd-observed-problems-') as tmp:
        for index, (name, cases) in enumerate(sorted(grouped.items())):
            source = corpus / 'coverage' / name
            manifest = json.loads((source / 'manifest.json').read_text(encoding='utf-8'))
            cf = Path(tmp) / f'{index}.cf'
            raw = base64.b64decode((source / 'configuration.cf.b64').read_bytes())
            if hashlib.sha256(raw).hexdigest() != manifest['configuration_cf']['decoded_sha256']:
                raise ValueError(f'{name}: CF hash differs')
            cf.write_bytes(raw)
            output = Path(tmp) / str(index)
            completed = subprocess.run([str(binary), 'cf', 'export', str(cf), str(output),
                                        '--source-version', manifest['evidence']['source_version']],
                                       capture_output=True, env=dict(os.environ, PATH='', LANG='ru_RU.UTF-8'))
            if completed.returncode:
                raise ValueError(f'{name}: exporter refused the witness set: {completed.stderr.decode(errors="replace")[-1500:]}')
            trees = {}
            for case in cases:
                file = case['file']
                if file not in trees:
                    # Missing files fail here rather than being called a symptom.
                    trees[file] = (parse_xml((source / 'native' / file).read_bytes()),
                                   parse_xml((output / file).read_bytes()))
                counts[classify(*trees[file], case)] += 1
    print('Observed problem witnesses: ' + ', '.join(f'{key}={value}' for key, value in sorted(counts.items())))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--ledger', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    run(args.corpus, json.loads(args.ledger.read_text(encoding='utf-8')), args.binary)
