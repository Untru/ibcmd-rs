"""Exercise only clean-room inputs; retain every generated fixture."""
import json, os, pathlib, subprocess, sys, uuid, zlib
if len(sys.argv) != 3:
    raise SystemExit('Usage: form_field_map_observer_cli_test.py BINARY EXTERNAL_TEST_ROOT')
exe = pathlib.Path(sys.argv[1]).resolve()
test_root = pathlib.Path(sys.argv[2]).resolve()
test_root.mkdir(parents=True, exist_ok=True)
root = test_root / ('ut-observer-cli-' + uuid.uuid4().hex)
root.mkdir()
checks = []

def q(s):
    return '"' + s.replace('"', '""') + '"'

def typed(t, v):
    return '{' + q(t) + ',' + (q(v) if t == 'S' else str(v)) + '}'
pairs = [('ManualQuery', typed('B', 1)), ('QueryText', typed('S', 'SELECT T.DeletionMark AS DeletionMark FROM Catalog.PRIVATE_QUERY_SENTINEL AS T')), ('FieldsMapItemId0', typed('N', 21)), ('FieldsMapItemName0', typed('S', 'DeletionMark')), ('FieldsMapItemId1', typed('N', 7)), ('FieldsMapItemName1', typed('S', 'DeletionMark')), ('ReqMapFieldId0', typed('N', 21)), ('PrivacySentinel', typed('S', 'PRIVATE_VALUE_SENTINEL'))]
bag = '{0,' + str(len(pairs)) + ',' + ','.join((q(k) + ',' + v for k, v in pairs)) + '}'
record = '{9,{17},0,"PRIVATE_ATTRIBUTE_SENTINEL",{1,0},{"Pattern",{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,' + bag + ',{0,0}}'
plain = '{4,{49,0},"PRIVATE_MODULE_SENTINEL",{4,1,' + record + '}}'

def packed(s):
    c = zlib.compressobj(wbits=-15)
    return c.compress(s.encode()) + c.flush()

def file(n, b):
    p = root / n
    p.write_bytes(b)
    return p

def run(name, args, code, error=None):
    before = {p.name for p in root.iterdir()}
    r = subprocess.run([str(exe), *map(str, args)], cwd=root, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
    after = {p.name for p in root.iterdir()}
    assert before == after, (name, 'unexpected filesystem output')
    assert r.returncode == code, (name, r.returncode, r.stderr[:120])
    assert b'PRIVATE_' not in r.stdout + r.stderr, name
    if error is not None:
        assert r.stdout == b'' and r.stderr.strip() == error.encode(), (name, 'nonstatic failure')
    else:
        assert r.stderr == b'', (name, 'unexpected stderr')
    checks.append(name)
    return r.stdout
valid = file('PRIVATE_INPUT_SENTINEL.bin', packed(plain))
out = run('valid-private-sentinels', [valid], 0)
s = json.loads(out)
allowed = {'schema_version', 'full_typed_universe_available', 'manual_dynamic_list_groups', 'ordinal', 'candidate_count', 'required_candidate_count', 'duplicate_remembered_names_across_ids', 'multiple_candidates_required', 'physical_order_matches_numeric_suffix_order', 'numeric_suffix_order_matches_numeric_id_order', 'query_selection_available', 'query_contains_wildcards', 'candidates', 'required', 'existing_shadowed_verdict', 'exact_selected_alias_present', 'twin_selected_alias_present'}

def shape(v):
    if isinstance(v, dict):
        assert set(v) <= allowed
        [shape(x) for x in v.values()]
    elif isinstance(v, list):
        [shape(x) for x in v]
    else:
        assert v is None or type(v) in [bool, int], type(v)
shape(s)
assert s['schema_version'] == 1 and s['full_typed_universe_available'] is False
g, = s['manual_dynamic_list_groups']
assert g['candidate_count'] == 2 and g['required_candidate_count'] == 1 and (g['duplicate_remembered_names_across_ids'] is True) and (g['multiple_candidates_required'] is False)
assert g['physical_order_matches_numeric_suffix_order'] is True and g['numeric_suffix_order_matches_numeric_id_order'] is False
assert all((c['exact_selected_alias_present'] is True for c in g['candidates']))
run('missing-argument', [], 1, 'FORM_OBSERVER_ARGUMENTS')
run('extra-argument', [valid, 'PRIVATE_ARG_SENTINEL'], 1, 'FORM_OBSERVER_ARGUMENTS')
run('missing-file', [root / 'PRIVATE_MISSING_SENTINEL'], 1, 'FORM_OBSERVER_INPUT')
run('invalid-deflate', [file('invalid.bin', b'PRIVATE_BAD_SENTINEL')], 1, 'FORM_OBSERVER_INVALID_FORM')
run('quoted-prefix-depth', [file('quoted.bin', packed('"' + '{' * 10000 + '0' + '}' * 10000 + '"' + plain))], 1, 'FORM_OBSERVER_INVALID_FORM')
run('deep-root', [file('deep.bin', packed('{' * 80 + '0' + '}' * 80))], 1, 'FORM_OBSERVER_LIMIT')
run('oversized-compressed', [file('oversize.bin', b'x' * (8 * 1024 * 1024 + 1))], 1, 'FORM_OBSERVER_LIMIT')
run('second-root', [file('second.bin', packed(plain + '{0}'))], 1, 'FORM_OBSERVER_INVALID_FORM')
bom = file('bom.bin', packed('\ufeff\r\n' + plain + ' \r\n'))
assert json.loads(run('valid-bom-whitespace', [bom], 0)) == s
if os.name != 'nt':
    link = root / 'linked.bin'
    link.symlink_to(valid)
    run('linked-input', [link], 1, 'FORM_OBSERVER_INPUT')
summary = {'checks': checks, 'passed': len(checks), 'failed': 0, 'fixturesRetained': str(root), 'privateSourceUsed': False, 'vmAction': False}
(root / 'summary.aggregate.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary))
