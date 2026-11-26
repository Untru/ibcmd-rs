#!/usr/bin/env python3
"""Opt-in public form/module corpus: native mutation, version RPC and DB immutability.

Requires absolute IBCMD_TEST_ROOT, IBCMD_TEST_NATIVE_BINARY, IBCMD_TEST_BINARY.
Retains all newly created artifacts; never opens an existing database.
"""
import base64
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
root, native, binary = [Path(os.environ[key]) for key in
    ['IBCMD_TEST_ROOT', 'IBCMD_TEST_NATIVE_BINARY', 'IBCMD_TEST_BINARY']]
assert all(path.is_absolute() for path in [root, native, binary])
root.mkdir(parents=True, exist_ok=True)
run = root / ('issue380-owners-' + uuid.uuid4().hex[:10])
run.mkdir()
(run / 'native-temp').mkdir()
fixture = repo / 'tests/fixtures/native-evidence/8.3.27.2214/form-table-representation-matrix'
(run / 'configuration.cf').write_bytes(base64.b64decode((fixture / 'configuration.cf.b64').read_bytes()))
common = [f'--db-path={run / "db"}', f'--data={run / "native-data"}', f'--temp={run / "native-temp"}']
commands = []
def command(tag, *args):
    argv = [str(native), 'infobase', *args, *common]
    result = subprocess.run(argv, cwd=run, capture_output=True, timeout=120)
    for kind in ['stdout', 'stderr']:
        (run / f'{tag}.{kind}').write_bytes(getattr(result, kind))
    commands.append(dict(tag=tag, argv=argv, code=result.returncode))
    (run / 'commands.json').write_text(json.dumps(commands, indent=2))
    print(tag, result.returncode, flush=True)
    assert result.returncode == 0, result.stderr.decode(errors='replace')

command('create', 'create', '--create-database', '--locale=ru_RU', f'--load={run / "configuration.cf"}', '--apply', '--force')
command('export-initial', 'config', 'export', str(run / 'initial'))
# Add an unrelated module using only the public tiny XML fixture.
original = 'ФайловаяБазаПроверка'
module_fixture = repo / 'tests/fixtures/file-infobase/tiny-source/CommonModules'
shutil.copytree(module_fixture / original, run / 'initial/CommonModules/Unrelated')
text = (module_fixture / (original + '.xml')).read_text(encoding='utf-8-sig')
text = text.replace(original, 'Unrelated').replace('6d1c2a0e-1cd0-4f00-9a00-000000000001', str(uuid.uuid4()))
(run / 'initial/CommonModules/Unrelated.xml').write_text(text, encoding='utf-8-sig')
configuration = run / 'initial/Configuration.xml'
text = configuration.read_text(encoding='utf-8-sig').replace('</ChildObjects>', '<CommonModule>Unrelated</CommonModule></ChildObjects>')
configuration.write_text(text, encoding='utf-8-sig')
command('import-initial', 'config', 'import', str(run / 'initial'))
command('baseline', 'config', 'export', str(run / 'baseline'))
command('baseline-status', 'config', 'export', 'status', f'--base={run / "baseline/ConfigDumpInfo.xml"}')
assert not (run / 'baseline-status.stdout').read_bytes()
(run / 'ibcmd-rs.toml').write_text('[[database]]\nname="owners"\nplatform="8.5.1"\ndb-path=' + json.dumps(str(run / 'db')) + '\n')

def rpc_status(tag):
    before = hashlib.sha256((run / 'db/1Cv8.1CD').read_bytes()).hexdigest()
    child = subprocess.Popen([str(binary), 'serve', '--stdio'], cwd=run, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        env={**os.environ, 'IBCMD_RS_CONFIG': str(run / 'ibcmd-rs.toml')})
    def call(number, method, params):
        body = json.dumps(dict(jsonrpc='2.0', id=number, method=method, params=params)).encode()
        child.stdin.write(f'Content-Length: {len(body)}\r\n\r\n'.encode() + body)
        child.stdin.flush()
        while True:
            headers = {}
            while True:
                line = child.stdout.readline()
                assert line, 'RPC closed before response'
                if line == b'\r\n': break
                key, value = line.decode().split(':', 1)
                headers[key.lower()] = value.strip()
            response = json.loads(child.stdout.read(int(headers['content-length'])))
            if response.get('id') == number:
                assert 'error' not in response, response
                return response['result']
    try:
        call(1, 'initialize', dict(protocolVersion='1.1', workspaceFolder=str(run)))
        result = call(2, 'objects/status', dict(infobase='owners', sourceDir=str(run / 'baseline'), versionBaselineDir=str(run / 'baseline')))
        (run / f'{tag}.json').write_text(json.dumps(result, indent=2))
    finally:
        child.kill()
        child.communicate()
    assert before == hashlib.sha256((run / 'db/1Cv8.1CD').read_bytes()).hexdigest()
    rows = {row['object']: row['databaseVersionStatus'] for row in result['objects']}
    assert rows and all(row['conclusive'] for row in rows.values()), result
    return rows

clean = rpc_status('rpc-clean')
assert all(row['changed'] is False for row in clean.values())
shutil.copytree(run / 'baseline', run / 'edit')
paths = ['Catalogs/CorpusList/Ext/ObjectModule.bsl', 'Catalogs/CorpusList/Forms/ListForm/Ext/Form/Module.bsl']
for path in paths:
    with (run / 'edit' / path).open('a') as stream:
        stream.write('\n// issue 380 native owner regression\n')
command('selected-import', 'config', 'import', 'files', f'--base-dir={run / "edit"}', '--partial', *paths)
command('modified-status', 'config', 'export', 'status', f'--base={run / "baseline/ConfigDumpInfo.xml"}')
changed = rpc_status('rpc-changed')
assert changed['Catalog.CorpusList']['changed'] is True
assert all(row['changed'] == (name == 'Catalog.CorpusList') for name, row in changed.items())
assert (run / 'modified-status.stdout').read_text(encoding='utf-8-sig').strip().splitlines() == [
    'modified: Catalog.CorpusList', 'modified: Catalog.CorpusList.Form.ListForm',
    'modified: Catalog.CorpusList.Form.ListForm.Form', 'modified: Catalog.CorpusList.ObjectModule']
(run / 'evidence.json').write_text(json.dumps(dict(run=str(run), clean=len(clean), changed={name: row['changed'] for name, row in changed.items()}, readOnly=True), indent=2))
print(run, flush=True)
