"""Closed informational project-context controls; no installed acceptance claim."""
import importlib.util,json,hashlib,tempfile,unittest,os,shutil
from unittest.mock import patch
from pathlib import Path
MODULE=Path(__file__).resolve().parents[2]/'scripts/edt-lab/oracle.py'
spec=importlib.util.spec_from_file_location('oracle_project_context',MODULE);oracle=importlib.util.module_from_spec(spec);spec.loader.exec_module(oracle)
class ProjectContextControls(unittest.TestCase):
 def setup_capture(self,root,name):
  run=root/name;run.mkdir();project=run/name
  argv=[str(root/'edt.exe'),'-data',str(run/'workspace'),'-timeout','120','-nl','en_US','-vmargs','-Xmx4g','-command','validate','--file',str(run/'pass.tsv'),'--project-list',str(project)]
  command={'argv':argv,'cwd':str(run),'exit_code':0}
  cp=run/'pass.command.json';cp.write_text(json.dumps(command),encoding='utf-8')
  return run,project,command,cp
 def counter(self,run,command_path,records):
  raw=''.join(f'!ENTRY {plugin} {severity} {code} 2026-10-02 04:00:00.000\n{body}\n' for plugin,severity,code,body in records).encode('utf-8')
  log=run/'phase.log';log.write_bytes(raw)
  value=oracle.warm_workspace_multiset(log,command_path)
  self.assertEqual(log.read_bytes(),raw)
  return value
 def test_all_eleven_templates_differ_only_in_bound_basename_and_keep_raw_hashes(self):
  self.assertEqual(len(oracle.WARM_PROJECT_CONTEXT_TEMPLATES),11)
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);counters=[]
   for name in ('project-copy','generated-edt-without-provenance'):
    run,project,command,cp=self.setup_capture(root,name)
    records=[(plugin,'1','0',prefix+project.name+suffix) for plugin,prefix,suffix in oracle.WARM_PROJECT_CONTEXT_TEMPLATES]
    counter,ledger=self.counter(run,cp,records);counters.append(counter)
    self.assertEqual(sum(counter.values()),11);self.assertEqual(len(ledger),11)
    self.assertTrue(all(row['kind']=='verified_project_context' for row in ledger))
    for row,record in zip(ledger,records):
     self.assertEqual(row['raw_message_and_stack'],record[3]);self.assertEqual(row['raw_body_sha256'],hashlib.sha256(record[3].encode()).hexdigest());self.assertEqual(row['command_sha256'],oracle.digest(cp))
   self.assertEqual(counters[0],counters[1])
   clean=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[0];existing=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[1]
   clean_counter,_=self.counter(run,cp,[(clean[0],'1','0',clean[1]+project.name+clean[2])]);existing_counter,_=self.counter(run,cp,[(existing[0],'1','0',existing[1]+project.name+existing[2])]);self.assertNotEqual(clean_counter,existing_counter)
 def test_unknown_plugin_severity_code_name_phase_suffix_stack_and_ai_are_unchanged(self):
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);run,project,command,cp=self.setup_capture(root,'project-copy')
   plugin,prefix,suffix=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[0];body=prefix+project.name+suffix
   variants=[('other.plugin','1','0',body),(plugin,'2','0',body),(plugin,'4','0',body),(plugin,'8','0',body),(plugin,'1','7',body),(plugin,'1','0',prefix+'other-project'+suffix),(plugin,'1','0',body+' extra'),(plugin,'1','0',body+'\n!STACK 0\ntrace'),(plugin,'1','0',body.replace('CLEAN_IMPORT','OTHER_IMPORT')),('com._1c.g5.v8.dt.lifecycle','1','0','!MESSAGE Starting phase UNKNOWN of context ProjectContext: '+project.name),('com._1c.g5.v8.dt.validation','1','0','!MESSAGE Found marker duplicates:\nPROJECT='+project.name),('com.e1c.edt.ai.ui.common','1','0','!MESSAGE AI response '+project.name)]
   for record in variants:
    with self.subTest(record=record[:3]):
     counter,ledger=self.counter(run,cp,[record]);self.assertEqual(counter,{record:1});self.assertEqual(ledger,[])
 def test_invalid_or_unbound_validate_command_never_rewrites_context(self):
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);run,project,command,cp=self.setup_capture(root,'project-copy')
   plugin,prefix,suffix=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[3];record=(plugin,'1','0',prefix+project.name+suffix)
   mutations=[lambda d:d.update(exit_code=False),lambda d:d.update(exit_code='0'),lambda d:d.update(exit_code=None),lambda d:d.update(exit_code=2),lambda d:d.update(timeout=True),lambda d:d.update(timeout_or_log_limit=True),lambda d:d['argv'].__setitem__(10,'export'),lambda d:d['argv'].__setitem__(13,'--project-name'),lambda d:d['argv'].append('other-project'),lambda d:d['argv'].__setitem__(14,'relative/project-copy'),lambda d:d['argv'].__setitem__(4,'0'),lambda d:d['argv'].__setitem__(8,'-Xmx4m'),lambda d:d.update(cwd='relative'),lambda d:d['argv'].__setitem__(14,str(root/'project-copy'))]
   for mutate in mutations:
    d=json.loads(json.dumps(command));mutate(d);cp.write_text(json.dumps(d),encoding='utf-8')
    counter,ledger=self.counter(run,cp,[record]);self.assertEqual(counter,{record:1});self.assertEqual(ledger,[])
 def test_multiplicity_and_phase_are_preserved(self):
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);run,project,command,cp=self.setup_capture(root,'project-copy');plugin,prefix,suffix=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[3];record=(plugin,'1','0',prefix+project.name+suffix)
   one,ledger=self.counter(run,cp,[record]);two,ledger2=self.counter(run,cp,[record,record]);self.assertEqual(sum((two-one).values()),1);self.assertEqual(len(ledger2),2)
   stopped=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[4];other,unused=self.counter(run,cp,[(stopped[0],'1','0',stopped[1]+project.name+stopped[2])]);self.assertNotEqual(one,other)
 def test_unicode_bound_basename_is_exact_not_a_substring(self):
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);run,project,command,cp=self.setup_capture(root,'Проект_тест');plugin,prefix,suffix=oracle.WARM_PROJECT_CONTEXT_TEMPLATES[3]
   counter,ledger=self.counter(run,cp,[(plugin,'1','0',prefix+project.name+suffix)]);self.assertEqual(len(ledger),1)
   body=prefix+project.name+'-another'+suffix;counter,ledger=self.counter(run,cp,[(plugin,'1','0',body)]);self.assertEqual(counter,{(plugin,'1','0',body):1});self.assertEqual(ledger,[])

class LegacyCaptureControls(unittest.TestCase):
 def legacy_fixture(self,root,historical22=False):
  lab=Path(os.environ.get('IBCMD_EDT_LAB',r'F:\ibcmd\lab\07'))
  source=lab/('validate-bsp83-warm-final-r2/harness-source.py' if historical22 else 'warm-phase-7735cb20-frozen-r2/scripts/edt-lab/oracle.py')
  if not source.is_file():self.skipTest('Reviewed historical lab harness unavailable; no substituted artifact code')
  known='e74311f5c501b907db7fa7386896b3efc376ba586db1226267cc381c32d97985' if historical22 else '55b6243112faec31ced67cf1465b0bec3af7fccc33995bc94fb18bfd47e7cf70'
  verified=source.read_bytes();self.assertEqual(hashlib.sha256(verified).hexdigest(),known)
  from types import ModuleType
  old=ModuleType('verified_old_fixture');old.__file__=str(source);exec(compile(verified,str(source),'exec'),old.__dict__)
  test_spec=importlib.util.spec_from_file_location('original_fixture_helpers',Path(__file__).with_name('edt_oracle_test.py'));helpers=importlib.util.module_from_spec(test_spec);test_spec.loader.exec_module(helpers)
  fixture=helpers.EvidenceControls();warm=fixture.warm_fixture
  raw=''.join(f'!ENTRY {plugin} 1 0 2026-10-02 04:00:00.000\n{prefix}project-copy{suffix}\n' for plugin,prefix,suffix in oracle.WARM_PROJECT_CONTEXT_TEMPLATES).encode()
  with patch.object(helpers,'oracle',old),patch.object(fixture,'warm_fixture',side_effect=lambda where:warm(where,logs=[raw]*3)):
   capture,args,project,original,evidence,calls=fixture.warm_binding_fixture(root)
  shutil.copy2(source,capture/'harness-source.py')
  path=capture/'invocation.json';invocation=json.loads(path.read_text(encoding='utf-8'));invocation['harness_sha256']=known;path.write_text(json.dumps(invocation),encoding='utf-8')
  args.run=root/'separate-derived-acceptance';args.run.mkdir();args.fifo_snapshots=False
  return capture,args,original,evidence
 def test_verified_original_full_capture_rebinds_before_new_closed_context_receipt(self):
  with tempfile.TemporaryDirectory() as folder:
   capture,args,original,evidence=self.legacy_fixture(Path(folder))
   before={p.relative_to(capture).as_posix():p.read_bytes() for p in capture.rglob('*') if p.is_file()}
   bound=oracle.bind_diagnostic_capture(capture,args,project_snapshot=oracle.snapshot(original))
   self.assertEqual(len(bound['warm_validation']['records']),3)
   for record in bound['warm_validation']['records']:self.assertEqual(len([r for r in record['workspace_lexical_context'] if r['kind']=='verified_project_context']),11)
   self.assertEqual({p.relative_to(capture).as_posix():p.read_bytes() for p in capture.rglob('*') if p.is_file()},before)
   self.assertFalse(list(capture.rglob('__pycache__')))
   receipt=json.loads(next(args.run.glob('warm-context-derived-*.json')).read_text(encoding='utf-8'))
   self.assertEqual(receipt['original_full_binding']['warm_validation'],evidence)
   self.assertEqual(receipt['original_warm_summary_sha256'],oracle.digest(capture/'validation-warm.json'))
 def test_known_22_capture_uses_same_verified_buffer_contract(self):
  with tempfile.TemporaryDirectory() as folder:
   capture,args,original,evidence=self.legacy_fixture(Path(folder),historical22=True)
   before={p.relative_to(capture).as_posix():p.read_bytes() for p in capture.rglob('*') if p.is_file()}
   with patch.object(oracle,'fair_file_operation',wraps=oracle.fair_file_operation) as tickets:
    bound=oracle.bind_diagnostic_capture(capture,args,project_snapshot=oracle.snapshot(original))
   self.assertTrue(any(call.args[0]=='legacy-snapshot' for call in tickets.call_args_list))
   self.assertEqual(len(bound['warm_validation']['records']),3)
   self.assertEqual({p.relative_to(capture).as_posix():p.read_bytes() for p in capture.rglob('*') if p.is_file()},before)
   self.assertFalse(list(capture.rglob('__pycache__')))
   receipt=json.loads(next(args.run.glob('warm-context-derived-*.json')).read_text(encoding='utf-8'))
   self.assertEqual(receipt['original_full_binding']['warm_validation'],evidence)
 def test_legacy_source_changed_during_scans_rejects_after_full_rebind(self):
  import contextlib
  with tempfile.TemporaryDirectory() as folder:
   capture,args,original,evidence=self.legacy_fixture(Path(folder),historical22=True)
   fair=oracle.fair_file_operation;changed=[]
   @contextlib.contextmanager
   def mutation(kind,**details):
    with fair(kind,**details) as receipt:yield receipt
    if kind=='legacy-snapshot' and not changed:
     source=capture/'harness-source.py';source.write_bytes(source.read_bytes()+b'\n# changed during scan\n');changed.append(True)
   with patch.object(oracle,'fair_file_operation',side_effect=mutation):
    with self.assertRaisesRegex(RuntimeError,'Historical harness changed during complete capture binding'):
     oracle.bind_legacy_warm_capture(args,capture,original)
   self.assertTrue(changed);self.assertFalse(list(args.run.glob('warm-context-derived-*.json')))
 def test_bad_known_hash_and_unknown_code_are_never_executed(self):
  for unknown in (False,True):
   with self.subTest(unknown=unknown),tempfile.TemporaryDirectory() as folder:
    root=Path(folder);capture,args,original,evidence=self.legacy_fixture(root);marker=root/'NEVER_EXECUTE'
    code='from pathlib import Path\nPath('+repr(str(marker))+').write_bytes(b"bad")\n'
    source=capture/'harness-source.py';source.write_text(code,encoding='utf-8')
    if unknown:
     path=capture/'invocation.json';d=json.loads(path.read_text(encoding='utf-8'));d['harness_sha256']=oracle.digest(source);path.write_text(json.dumps(d),encoding='utf-8')
    with self.assertRaises(RuntimeError):oracle.bind_diagnostic_capture(capture,args,project_snapshot=oracle.snapshot(original))
    self.assertFalse(marker.exists());self.assertFalse(list(args.run.glob('warm-context-derived-*.json')))
 def test_changed_original_raw_label_stack_severity_count_or_body_always_rejects(self):
  for mutation in ('label','stack','severity','count','body'):
   with self.subTest(mutation=mutation),tempfile.TemporaryDirectory() as folder:
    capture,args,original,evidence=self.legacy_fixture(Path(folder));path=capture/'edt-validate-pass-001.workspace-phase-log';raw=path.read_bytes()
    if mutation=='label':raw=raw.replace(b'project-copy',b'other-project')
    elif mutation=='stack':raw+=b'!STACK 0\nchanged trace\n'
    elif mutation=='severity':raw=raw.replace(b' 1 0 ',b' 4 0 ')
    elif mutation=='count':raw+=raw
    elif mutation=='body':raw=raw.replace(b'CLEAN_IMPORT',b'UNKNOWN_IMPORT')
    path.write_bytes(raw)
    with self.assertRaises(RuntimeError):oracle.bind_diagnostic_capture(capture,args,project_snapshot=oracle.snapshot(original))
    self.assertFalse(list(args.run.glob('warm-context-derived-*.json')))

if __name__=='__main__':unittest.main(verbosity=2)
