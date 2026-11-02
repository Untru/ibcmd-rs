"""Public frozen-context controls. Every input and log is retained; no private data."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid
import zlib

if os.environ.get("IBCMD_TEST_KEEP_FIXTURES") != "1":
    raise SystemExit("IBCMD_TEST_KEEP_FIXTURES=1 is required")
if len(sys.argv) != 3:
    raise SystemExit("Usage: form_field_map_context_cli_test.py BINARY RETAINED_PARENT")
exe = Path(sys.argv[1]).resolve()
parent = Path(sys.argv[2]).resolve()
parent.mkdir(parents=True, exist_ok=True)
root = parent / ("context-" + uuid.uuid4().hex)
root.mkdir()
checks = []


def save(name, data):
    path = root / name
    with path.open("xb") as output:
        output.write(data)
    return path


def quoted(value):
    return '"' + value.replace('"', '""') + '"'


def record(name):
    query = "SELECT T.ПометкаУдаления AS ПометкаУдаления FROM Catalog.PRIVATE_TABLE_SENTINEL AS T"
    pairs = [('ManualQuery', '{"B",1}'), ('QueryText', '{"S",' + quoted(query) + '}'),
             ('FieldsMapItemId0', '{"N",1}'), ('FieldsMapItemName0', '{"S","DeletionMark"}'),
             ('ReqMapFieldId0', '{"N",1}')]
    bag = '{0,' + str(len(pairs)) + ',' + ','.join(quoted(k) + ',' + v for k, v in pairs) + '}'
    return ('{9,{17},0,' + quoted(name) + ',{1,0},{"Pattern",{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}},'
            '{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,' + bag + ',{0,0}}')


def digest(data):
    return hashlib.sha256(data).hexdigest()


plain = '{4,{49,0},"PRIVATE_MODULE_SENTINEL",{4,2,' + record('PRIVATE_SECOND') + ',' + record('PRIVATE_FIRST') + '}}'
c = zlib.compressobj(wbits=-15)
body_bytes = c.compress(plain.encode()) + c.flush()
native_bytes = b'<public-native-binding/>'
candidate_bytes = b'<public-candidate-binding/>'
body = save('PRIVATE_BODY.bin', body_bytes)
native = save('PRIVATE_NATIVE.xml', native_bytes)
candidate = save('PRIVATE_CANDIDATE.xml', candidate_bytes)
context = {'schema_version': 1, 'body_sha256': digest(body_bytes),
           'native_xml_sha256': digest(native_bytes), 'candidate_xml_sha256': digest(candidate_bytes),
           'attributes': [{'affected_xml_ordinal': i, 'attribute_name': name,
                           'native_main_table': 'Catalog.PRIVATE_TABLE_SENTINEL',
                           'candidate_main_table': 'Catalog.PRIVATE_TABLE_SENTINEL'}
                          for i, name in [(1, 'PRIVATE_FIRST'), (2, 'PRIVATE_SECOND')]]}


def run(label, context_bytes, error=None, native_path=native, extra=None):
    ctx = save(label + '.context.json', context_bytes)
    before = {p.name: digest(p.read_bytes()) for p in root.iterdir() if p.is_file()}
    args = [str(exe), str(body), '--context', str(ctx), str(native_path), str(candidate)]
    if extra:
        args += extra
    result = subprocess.run(args, cwd=root, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
    after = {p.name: digest(p.read_bytes()) for p in root.iterdir() if p.is_file()}
    # Capture failed controls too; diagnostic fixtures/logs are always retained.
    save(label + '.stdout', result.stdout)
    save(label + '.stderr', result.stderr)
    assert before == after, (label, 'input mutation or new file')
    assert b'PRIVATE' not in result.stdout + result.stderr, label
    if error:
        assert result.returncode == 1 and result.stdout == b'' and result.stderr.strip() == error.encode(), label
    else:
        assert result.returncode == 0 and result.stderr == b'', label
    checks.append(label)
    return result.stdout


encoded = json.dumps(context).encode()
report = json.loads(run('valid-exact-reordered-join', encoded))
assert report['schema_version'] == 2 and report['full_typed_universe_available'] is False
assert [(g['affected_xml_ordinal'], g['raw_group_ordinal']) for g in report['context_bound_groups']] == [(1, 2), (2, 1)]
for group in report['context_bound_groups']:
    query = group['query']
    assert query['source_match'] == 'unique' and query['semantic_unselected_proven'] is False
    cell, = [v for v in query['observations']['identity_paths']
             if v['source_field'] == 'deletion_mark_russian' and v['result_alias'] == 'russian']
    assert cell['occurrences'] == {'exact': 1, 'case_folded': 1}
for key in ['body_sha256', 'native_xml_sha256', 'candidate_xml_sha256']:
    altered = json.loads(encoded)
    altered[key] = '0' * 64
    run('wrong-' + key, json.dumps(altered).encode(), 'FORM_OBSERVER_INVALID_CONTEXT')
for label, key, value in [('duplicate-name', 'attribute_name', 'PRIVATE_FIRST'),
                          ('missing-name', 'attribute_name', 'PRIVATE_MISSING'),
                          ('duplicate-ordinal', 'affected_xml_ordinal', 1),
                          ('conflicting-main-table', 'candidate_main_table', 'Catalog.OTHER')]:
    altered = json.loads(encoded)
    altered['attributes'][1][key] = value
    run(label, json.dumps(altered).encode(), 'FORM_OBSERVER_INVALID_CONTEXT')
run('malformed-json', b'{"PRIVATE_BAD":', 'FORM_OBSERVER_INVALID_CONTEXT')
run('context-byte-cap', b'x' * (64 * 1024 + 1), 'FORM_OBSERVER_LIMIT')
large = save('oversized-public-xml.xml', b'x' * (8 * 1024 * 1024 + 1))
run('xml-byte-cap', encoded, 'FORM_OBSERVER_LIMIT', native_path=large)
run('context-extra-argument', encoded, 'FORM_OBSERVER_ARGUMENTS', extra=['PRIVATE_EXTRA'])
summary = {'checks': checks, 'passed': len(checks), 'failed': 0, 'privateSourceUsed': False,
           'vmAction': False, 'retainedFixture': str(root)}
save('summary.aggregate.json', (json.dumps(summary, indent=2) + '\n').encode())
print(json.dumps(summary))
