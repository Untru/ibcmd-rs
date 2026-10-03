"""Immutable offline evidence index; no subprocess, SQL, lifecycle or source edits."""
import hashlib, json, pathlib, stat

lab=pathlib.Path('F:/ibcmd/lab/05/wave3/platform85')
output=lab/'logs/extension-v4-result-checkpoint.json'
assert not output.exists()
base=lab/'logs/extension-control-frozen-v4.json'
cleanup=lab/'logs/extension-v4-cleanup-frozen-v1.json'
assert hashlib.sha256(base.read_bytes()).hexdigest().upper()=='D4E662C47961B66C268BD3A2E36FEE1B28BA6B2DE0CB4DA6D0F02E18AAB30B5E'
assert hashlib.sha256(cleanup.read_bytes()).hexdigest().upper()=='A75A435230FAE3DE6806465A25FA72BB580797BECF2DF7416D6939C9AEE9E410'
paths={base,cleanup,lab/'tools/freeze_extension_v4_result.py',lab/'EXTENSION-V4-RESULT.md'}
for manifest in [base,cleanup]:
    paths.update(pathlib.Path(f['file']) for f in json.loads(manifest.read_bytes())['files'])
for root in [lab/'ext-version-native-v2',lab/'extension-v4-cleanup-v1',lab/'snapshots/ext_version_native_v2_cold_prestart',lab/'snapshots/ext_version_native_v2_warm_before',lab/'snapshots/ext_version_native_v4_postfailed']:
    paths.update(p for p in root.rglob('*') if p.is_file())
paths.update(lab/'obs'/('ext-version-native-old-v2.'+suffix) for suffix in ['log','pid','identity.json','client-out.txt'])
paths.update(lab/'logs'/name for name in ['extension-v4-execution.log','extension-v4-cleanup-execution.log','extension-v4-cleanup-postcheck.json','extension-v4-postfailed-full-physical-delta.json','extension-v4-postfailed-delta-readonly.log','extension-v4-cleanup-pure-final.log','extension-v4-cleanup-closure-readonly.log'])
postcheck=json.loads((lab/'logs/extension-v4-cleanup-postcheck.json').read_bytes())
paths.add(pathlib.Path(postcheck['stopped_state']))
for pattern in ['before-old-*.log','before-native-import-*.log','before-signal-*.log']:
    paths.update((lab/'cluster').glob(pattern))
records=[]
for path in sorted(paths):
    assert path.is_absolute() and path.is_file()
    for ancestor in [path,*path.parents]:
        assert not(getattr(ancestor.lstat(),'st_file_attributes',0)&stat.FILE_ATTRIBUTE_REPARSE_POINT)
    digest=hashlib.sha256()
    with path.open('rb') as stream:
        while chunk:=stream.read(65536):digest.update(chunk)
    records.append(dict(file=str(path),bytes=path.stat().st_size,sha256=digest.hexdigest()))
failed=json.loads((lab/'ext-version-native-v2/010-native-import.json').read_bytes())
final=json.loads((lab/'extension-v4-cleanup-v1/final-state.json').read_bytes())
assert failed['ExitCode']==-1 and final['guarded_stop_clean'] and not final['registered_or_unconfirmed'] and not final['worker_held_or_unconfirmed']
assert postcheck['private_state_absent'] and postcheck['own_worker_lease_absent'] and not postcheck['exact_saved_process_identities_alive']
delta=json.loads((lab/'logs/extension-v4-postfailed-full-physical-delta.json').read_bytes())
assert all(not(v['added'] or v['removed'] or v['changed']) for v in delta['storage'].values())
assert all(not(v['added'] or v['removed']) for v in delta['auxiliary'].values())
record=dict(format=1,scope='Actual native-only extension Version V4 failed import plus exact owned cleanup; no activation or product capability acceptance',database='ibcmd_rs_05_p85_w3_ext_version_native_20261001',platform='8.5.1.1150',extension='ServiceDesk',fixture_version_only='1.8.3.0->1.8.3.1',native_import_exit=-1,native_apply_executed=False,new_observer_executed=False,old_session_observed='SID1 1.8.3.0/1.8.3.0',failure_predefined_references=['Enum.СтатусыИзвлеченияТекстаФайлов.EmptyRef','Catalog.Пользователи.EmptyRef'],full_warm_to_postfailure_six_storage_unchanged=True,full_warm_to_postfailure_eight_auxiliary_unchanged=True,comparison_normalization='none; pack offsets identify capture locations only',cleanup_exit=0,cleanup_final=final,product_extension_capability_enabled=False,CFE_built=False,source_reference='288193085e473c852e64cc5a86ca825bca0113b0',rust_CLI_executed=False,files=records)
with output.open('x',encoding='utf-8') as stream:
    json.dump(record,stream,ensure_ascii=False,indent=2)
    stream.write('\n')
print(json.dumps({'checkpoint':str(output),'files':len(records),'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'native_version_control':'FAILED before apply; cleanup proven clean','no_new_runtime':True}))
