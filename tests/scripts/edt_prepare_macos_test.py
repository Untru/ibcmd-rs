#!/usr/bin/env python3
"""Retained controls for lab path, cancellation and capture-evidence guards."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/edt-lab'))
import prepare_macos as lab


class Controls(unittest.TestCase):
    def setUp(self):
        root = Path(os.environ.get('TMPDIR', '/tmp'))
        if not root.is_relative_to(lab.LAB):
            self.skipTest('Source lab-env.sh; control fixtures must stay external')
        self.run = Path(tempfile.mkdtemp(prefix='issue358-control-', dir=root))
        # Fixtures intentionally retained; no teardown/permanent removal.

    def test_rejects_internal_path(self):
        with self.assertRaises(lab.OracleError):
            lab.external(Path('/Users/example/workspace'))

    def test_rejects_traversal(self):
        with self.assertRaises(lab.OracleError):
            lab.external(lab.LAB / 'tmp/../escape')

    def test_rejects_link(self):
        link = self.run / 'link'
        link.symlink_to(self.run / 'missing')
        with self.assertRaises(lab.OracleError):
            lab.external(link)

    def test_whole_bundle_overlap_rejected_before_any_capture_output(self):
        bundle = self.run / 'installed.app'
        edt = bundle / 'Contents/Eclipse/1cedtcli'
        edt.parent.mkdir(parents=True)
        for directory in ('Contents/MacOS', 'Resources'):
            (bundle / directory).mkdir(parents=True)
        java = self.run / 'java'
        for relative in ('capture', 'Contents/capture', 'Contents/MacOS/capture',
                         'Contents/Eclipse/capture', 'Resources/capture'):
            with self.subTest(relative=relative):
                run = bundle / relative
                args = SimpleNamespace(run=run, edt=edt, java=java)
                with patch.object(lab.sys, 'platform', 'darwin'), \
                     patch.object(lab, 'isolated_environment') as environment, \
                     patch.object(lab, 'snapshot') as inventory, \
                     patch.object(lab.subprocess, 'Popen') as launch:
                    with self.assertRaisesRegex(lab.OracleError, 'Whole EDT app bundle'):
                        lab.capture(args)
                self.assertFalse(run.exists())
                environment.assert_not_called()
                inventory.assert_not_called()
                launch.assert_not_called()

    def test_bundle_ancestor_rejected_and_sibling_capture_allowed(self):
        edt = self.run / 'installed.app/Contents/Eclipse/1cedtcli'
        for run in (self.run, self.run / 'installed.app'):
            with self.subTest(run=run):
                with self.assertRaises(lab.OracleError):
                    lab.require_disjoint_installation(run, edt)
        lab.require_disjoint_installation(self.run / 'capture', edt)
        lab.require_disjoint_installation(self.run / 'installed.app-capture', edt)

    def test_physical_bundle_alias_rejected(self):
        bundle = self.run / 'installed.app'
        edt = bundle / 'Contents/Eclipse/1cedtcli'
        edt.parent.mkdir(parents=True)
        alias = self.run / 'INSTALLED.APP'
        if not alias.exists():
            self.skipTest('Volume uses case-sensitive directory names')
        with self.assertRaises(lab.OracleError):
            lab.require_disjoint_installation(alias / 'Contents/capture', edt)

    def test_non_bundle_launcher_layout_rejected(self):
        with self.assertRaises(lab.OracleError):
            lab.require_disjoint_installation(self.run / 'capture', self.run / 'tool/1cedtcli')

    def test_environment_redirects_all_data_and_removes_java_injection(self):
        with patch.dict(os.environ, {'JAVA_TOOL_OPTIONS': '-Duser.home=/internal'}):
            env, receipt = lab.isolated_environment(self.run)
        self.assertNotIn('JAVA_TOOL_OPTIONS', env)
        for key in ('HOME', 'TMPDIR', 'TEMP', 'TMP', 'XDG_CACHE_HOME', 'XDG_CONFIG_HOME',
                    'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            self.assertTrue(Path(receipt[key]).is_relative_to(self.run))
            self.assertEqual(env[key], receipt[key])

    def test_installation_binds_actual_launcher_and_bundle_selection(self):
        with patch.object(lab, 'snapshot', return_value={'files': []}), \
             patch.object(lab, 'digest', return_value='a' * 64), \
             patch.object(lab, 'file_record', side_effect=lambda p: {'path': str(p)}):
            value = lab.installation(self.run / 'Contents/Eclipse/1cedtcli', self.run / 'jdk/bin/java')
        paths = [row['path'] for row in value['runtime_files']]
        self.assertIn(str(self.run / 'Contents/MacOS/1cedt'), paths)
        self.assertIn(str(self.run / 'Contents/Eclipse/configuration/org.eclipse.equinox.simpleconfigurator/bundles.info'), paths)
        self.assertEqual(value['status'], 'INVENTORIED_NOT_RUNTIME_VERIFIED')

    def test_failed_command_preserves_exit_and_raw_receipts(self):
        process = Mock()
        process.wait.return_value = 7
        with patch.object(lab.subprocess, 'Popen', return_value=process):
            with self.assertRaises(lab.OracleError):
                lab.command(self.run, 'failed', ['fake'], 1, {}, {})
        record = json.loads((self.run / 'failed.command.json').read_text())
        self.assertEqual(record['exit_code'], 7)
        self.assertTrue((self.run / 'failed.stdout').is_file())

    def test_cancellation_kills_and_reaps_owned_process_group(self):
        process = Mock(pid=12345)
        process.wait.side_effect = [KeyboardInterrupt(), -9]
        with patch.object(lab.subprocess, 'Popen', return_value=process), \
             patch.object(lab.os, 'killpg') as kill:
            with self.assertRaises(KeyboardInterrupt):
                lab.command(self.run, 'cancelled', ['fake'], 1, {}, {})
        kill.assert_called_once_with(12345, lab.signal.SIGKILL)
        record = json.loads((self.run / 'cancelled.command.json').read_text())
        self.assertEqual(record['exit_code'], -9)
        self.assertEqual(record['error_type'], 'KeyboardInterrupt')

    def test_changed_installation_prevents_terminal_discovery_receipt(self):
        edt = self.run / 'tool.app/Contents/Eclipse/1cedtcli'
        edt.parent.mkdir(parents=True)
        edt.write_bytes(b'fake')
        edt.chmod(0o700)
        (edt.parent / '1cedt.ini').write_text('-startup\n../Eclipse/plugins/launcher.jar\n-vmargs\n-Xmx1g\n')
        java = self.run / 'java'
        java.write_bytes(b'fake')
        java.chmod(0o700)
        run = self.run / 'capture'
        args = SimpleNamespace(run=run, edt=edt, java=java, edt_build='2026.2.1.3',
            mode='discover', timeout=1, heap_gib=1, jvm_library=java)
        before = {'runtime_files': [], 'payload': 'before'}
        after = {'runtime_files': [], 'payload': 'changed'}
        with patch.object(lab.sys, 'platform', 'darwin'), \
             patch.object(lab.fcntl, 'flock'), \
             patch.object(lab, 'installation', side_effect=[before, after]), \
             patch.object(lab, 'command', return_value=b'2026.2.1.3'):
            with self.assertRaises(lab.OracleError):
                lab.capture(args)
        self.assertIn('-vm\n' + str(java) + '\n', (run / '1cedt.ini').read_text())
        self.assertFalse((run / 'result.json').exists())
        self.assertFalse((run / 'prepared.json').exists())
        self.assertTrue((run / 'failure.json').is_file())
        self.assertTrue((run / 'installation-after.json').is_file())

    def test_timeout_has_no_positive_command_status(self):
        process = Mock(pid=12346)
        process.wait.side_effect = [subprocess.TimeoutExpired('fake', 1), -9]
        with patch.object(lab.subprocess, 'Popen', return_value=process), \
             patch.object(lab.os, 'killpg'):
            with self.assertRaises(subprocess.TimeoutExpired):
                lab.command(self.run, 'timeout', ['fake'], 1, {}, {})
        self.assertEqual(json.loads((self.run / 'timeout.command.json').read_text())['error_type'], 'TimeoutExpired')


if __name__ == '__main__':
    unittest.main()
