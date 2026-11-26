"""Synthetic journal tests; no native tools/SQL or destructive teardown."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2] / 'scripts/platform-lab/acceptance.py'
spec = importlib.util.spec_from_file_location('platform_acceptance', SOURCE)
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class Acceptance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.environ.get('IBCMD_TEST_KEEP_FIXTURES') != '1':
            raise RuntimeError('Set IBCMD_TEST_KEEP_FIXTURES=1; fixtures are preserved')
        cls.root = Path(tempfile.mkdtemp(prefix='issue361-synthetic-', dir=os.environ['TMPDIR']))
        cls.counter = 0

    def ref(self, value):
        type(self).counter += 1
        path = self.root / f'{self.counter}.json'
        path.write_text(json.dumps(value) if not isinstance(value, str) else value)
        return {'path': path.name, 'sha256': audit.digest(path)}

    def row(self, release, export_difference=False):
        build = release + '.1234'
        blank = self.ref('')
        snapshot = self.ref({'synthetic': True})
        probe = self.ref({'platform_build': build, 'binary_sha256': 'a' * 64, 'exit_code': 0,
                          'stdout': self.ref('ibcmd ' + build), 'stderr': blank})
        empty = self.ref({'engine': 'mssql', 'platform_build': build,
                          'native_target_id': 'native', 'candidate_target_id': 'candidate',
                          'native_empty': True, 'candidate_empty': True,
                          'native_inventory': snapshot, 'candidate_inventory': snapshot})
        row = {'release': release, 'state': 'measured', 'platform_build': build,
               'native': {'binary_sha256': 'a' * 64, 'version_probe': probe},
               'candidate': {'binary_sha256': 'b' * 64, 'git_commit': 'c' * 40, 'clean_worktree': True},
               'corpus': {'id': 'synthetic', 'version': '1', 'origin': 'unit test',
                          'authorization': 'synthetic only', 'snapshot': snapshot},
               'database': {'engine': 'mssql', 'source_id': 'source', 'native_target_id': 'native',
                            'candidate_target_id': 'candidate', 'source_before': snapshot,
                            'source_after': snapshot, 'empty_targets_proof': empty},
               'observations': {'unknown_layouts': [], 'write_limits': ['no measured production writes'],
                                'layout_census': snapshot, 'write_policy': snapshot},
               'phases': {}, 'comparisons': {}}
        records = {}
        for phase in audit.PHASES:
            database_id = ('source' if phase.endswith('_source') else
                           'native' if phase in ('native_import_empty', 'native_reexport_native_target') else 'candidate')
            record = {
                'phase': phase, 'platform_build': build, 'candidate_commit': 'c' * 40,
                'binary_sha256': ('b' if phase.startswith('candidate_') else 'a') * 64,
                'corpus_snapshot_sha256': snapshot['sha256'], 'exit_code': 0,
                'argv_redacted': ['synthetic'], 'started_utc': '2026-10-23T00:00:00Z',
                'ended_utc': '2026-10-23T00:00:01Z', 'stdout': blank, 'stderr': blank,
                'output_inventory': self.ref({'phase': phase, 'synthetic': True, 'files': [{
                    'path': 'Configuration.xml' if phase.endswith('_source') else 'Catalogs/Products.xml',
                    'sha256': ('f' if export_difference and phase == 'candidate_export_source' else
                               'd' if phase.endswith('_source') else 'e') * 64, 'size_bytes': 4}]}),
                'database_id': database_id}
            records[phase] = record
        for phase, record in records.items():
            if phase.endswith('_source'):
                input_hash = snapshot['sha256']
            elif phase.endswith('_empty'):
                input_hash = records['native_export_source']['output_inventory']['sha256']
            else:
                import_phase = ('native_import_empty' if phase == 'native_reexport_native_target' else 'candidate_import_empty')
                input_hash = records[import_phase]['output_inventory']['sha256']
            record['input_inventory_sha256'] = input_hash
            row['phases'][phase] = self.ref(record)
        for kind in audit.COMPARISONS:
            sides = (('native_export_source', 'candidate_export_source') if kind == 'export' else
                     ('native_reexport_native_target', 'native_reexport_candidate_target'))
            left_ref = records[sides[0]]['output_inventory']
            right_ref = records[sides[1]]['output_inventory']
            left = audit.load(self.root / left_ref['path'])['files'][0]
            right = audit.load(self.root / right_ref['path'])['files'][0]
            different = kind == 'export' and export_difference
            summary = {'left_only': 0, 'right_only': 0, 'different': int(different), 'unchanged': int(not different)}
            diff = self.ref({'summary': summary, 'left_inventory_sha256': left_ref['sha256'],
                             'right_inventory_sha256': right_ref['sha256'], 'differences': [{
                                 'path': left['path'], 'status': 'different' if different else 'unchanged',
                                 'left_sha256': left['sha256'], 'right_sha256': right['sha256'],
                                 'left_size_bytes': 4, 'right_size_bytes': 4}]})
            row['comparisons'][kind] = self.ref({
                'scope': 'full', 'platform_build': build, 'candidate_commit': 'c' * 40,
                'corpus_snapshot_sha256': snapshot['sha256'], 'full_diff': diff, 'summary': summary,
                'left_inventory_sha256': records[sides[0]]['output_inventory']['sha256'],
                'right_inventory_sha256': records[sides[1]]['output_inventory']['sha256']})
        return row

    def document(self):
        return {'schema_version': 1, 'rows': [self.row(release) for release in audit.RELEASES]}

    def rewrite(self, ref, **changes):
        value = audit.load(self.root / ref['path'])
        value.update(changes)
        return self.ref(value)

    def test_complete_synthetic_journals_only_become_ready_for_review(self):
        result = audit.check(self.document(), self.root)
        self.assertEqual(result['state'], 'ready_for_review')
        self.assertEqual(result['native_acceptance'], 'not_determined')

    def test_differences_and_limits_remain_visible_for_review(self):
        row = self.row('8.3.24', export_difference=True)
        row['observations']['unknown_layouts'] = ['unrecognized catalog layout']
        result = audit.check_row(row, self.root)
        self.assertEqual(result['differences'], 1)
        self.assertEqual(result['unknown_layouts'], row['observations']['unknown_layouts'])
        self.assertEqual(result['write_limits'], row['observations']['write_limits'])

    def test_stale_same_count_diff_is_rejected_even_when_outer_hashes_are_correct(self):
        row = self.row('8.3.24')
        export = audit.load(self.root / row['comparisons']['export']['path'])
        import_ref = row['comparisons']['import']
        row['comparisons']['import'] = self.rewrite(import_ref, full_diff=export['full_diff'])
        with self.assertRaisesRegex(audit.EvidenceError, 'full diff inventory binding'):
            audit.check_row(row, self.root)
        # Rebinding just the stale nested labels still cannot hide its unrelated paths/content.
        imported = audit.load(self.root / import_ref['path'])
        relabeled = self.rewrite(export['full_diff'],
                                 left_inventory_sha256=imported['left_inventory_sha256'],
                                 right_inventory_sha256=imported['right_inventory_sha256'])
        row['comparisons']['import'] = self.rewrite(import_ref, full_diff=relabeled)
        with self.assertRaisesRegex(audit.EvidenceError, 'diff path absent'):
            audit.check_row(row, self.root)

    def test_same_path_stale_content_size_or_status_is_rejected(self):
        for changes in ({'left_sha256': 'a' * 64}, {'right_sha256': 'a' * 64},
                        {'left_size_bytes': 9}, {'right_size_bytes': True}, {'status': 'different'}):
            row = self.row('8.3.25')
            ref = row['comparisons']['import']
            report = audit.load(self.root / ref['path'])
            diff = audit.load(self.root / report['full_diff']['path'])
            diff['differences'][0].update(changes)
            row['comparisons']['import'] = self.rewrite(ref, full_diff=self.ref(diff))
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)

    def test_full_diff_accepts_all_four_statuses_bound_to_real_inventory_entries(self):
        row = self.row('8.3.26')
        left = [{'path': name, 'sha256': code * 64, 'size_bytes': 4}
                for name, code in (('Same.xml', 'a'), ('Changed.xml', 'b'), ('Left.xml', 'c'))]
        right = [{'path': name, 'sha256': code * 64, 'size_bytes': 4}
                 for name, code in (('Same.xml', 'a'), ('Changed.xml', 'd'), ('Right.xml', 'e'))]
        refs = [self.ref({'files': files}) for files in (left, right)]
        for phase, ref in zip(('native_reexport_native_target', 'native_reexport_candidate_target'), refs):
            row['phases'][phase] = self.rewrite(row['phases'][phase], output_inventory=ref)
        entries = [
            {'path': 'Same.xml', 'status': 'unchanged', 'left_sha256': 'a' * 64,
             'right_sha256': 'a' * 64, 'left_size_bytes': 4, 'right_size_bytes': 4},
            {'path': 'Changed.xml', 'status': 'different', 'left_sha256': 'b' * 64,
             'right_sha256': 'd' * 64, 'left_size_bytes': 4, 'right_size_bytes': 4},
            {'path': 'Left.xml', 'status': 'left_only', 'left_sha256': 'c' * 64,
             'right_sha256': None, 'left_size_bytes': 4, 'right_size_bytes': None},
            {'path': 'Right.xml', 'status': 'right_only', 'left_sha256': None,
             'right_sha256': 'e' * 64, 'left_size_bytes': None, 'right_size_bytes': 4}]
        summary = dict.fromkeys(('left_only', 'right_only', 'different', 'unchanged'), 1)
        diff = self.ref({'summary': summary, 'differences': entries,
                         'left_inventory_sha256': refs[0]['sha256'],
                         'right_inventory_sha256': refs[1]['sha256']})
        row['comparisons']['import'] = self.rewrite(row['comparisons']['import'],
            full_diff=diff, summary=summary, left_inventory_sha256=refs[0]['sha256'],
            right_inventory_sha256=refs[1]['sha256'])
        self.assertEqual(audit.check_row(row, self.root)['differences'], 3)

    def test_source_inventory_rejects_malformed_or_duplicate_entries(self):
        valid = {'path': 'Configuration.xml', 'sha256': 'a' * 64, 'size_bytes': 4}
        documents = [{'files': None}, {'files': [None]}, {'files': [valid, valid]}]
        for changes in ({'path': '../outside'}, {'sha256': 'invalid'},
                        {'size_bytes': -1}, {'size_bytes': True}):
            documents.append({'files': [{**valid, **changes}]})
        for document in documents:
            with self.assertRaises(audit.EvidenceError):
                audit.source_inventory(self.root, self.ref(document))

    def test_diff_cannot_omit_inventory_paths_or_duplicate_entries(self):
        for omit in (True, False):
            row = self.row('8.3.26')
            ref = row['comparisons']['export']
            report = audit.load(self.root / ref['path'])
            diff = audit.load(self.root / report['full_diff']['path'])
            diff['differences'] = [] if omit else diff['differences'] * 2
            row['comparisons']['export'] = self.rewrite(ref, full_diff=self.ref(diff))
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)

    def test_missing_release_or_duplicate_is_rejected(self):
        doc = self.document()
        doc['rows'][1] = copy.deepcopy(doc['rows'][0])
        with self.assertRaisesRegex(audit.EvidenceError, 'one row per'):
            audit.check(doc, self.root)
        doc['rows'].pop()
        with self.assertRaisesRegex(audit.EvidenceError, 'three release'):
            audit.check(doc, self.root)

    def test_nearby_build_and_mismatched_exact_probe_are_rejected(self):
        row = self.row('8.3.24')
        row['platform_build'] = '8.3.27.2214'
        with self.assertRaisesRegex(audit.EvidenceError, 'does not match'):
            audit.check_row(row, self.root)
        row['platform_build'] = '8.3.24.9999'
        with self.assertRaisesRegex(audit.EvidenceError, 'exact binary/build'):
            audit.check_row(row, self.root)

    def test_wrong_probe_stdout_is_rejected_even_with_valid_hash(self):
        row = self.row('8.3.24')
        row['native']['version_probe'] = self.rewrite(row['native']['version_probe'], stdout=self.ref('8.3.24.9999'))
        with self.assertRaisesRegex(audit.EvidenceError, 'one exact build'):
            audit.check_row(row, self.root)

    def test_native_import_and_native_reexport_are_mandatory(self):
        for phase in ('native_import_empty', 'native_reexport_candidate_target'):
            row = self.row('8.3.24')
            del row['phases'][phase]
            with self.assertRaisesRegex(audit.EvidenceError, 'incomplete'):
                audit.check_row(row, self.root)

    def test_unrelated_diff_database_or_input_is_rejected(self):
        row = self.row('8.3.24')
        row['comparisons']['import'] = row['comparisons']['export']
        with self.assertRaisesRegex(audit.EvidenceError, 'output binding'):
            audit.check_row(row, self.root)
        for changes in ({'database_id': 'unrelated'}, {'input_inventory_sha256': 'f' * 64}):
            row = self.row('8.3.24')
            phase = 'candidate_import_empty'
            row['phases'][phase] = self.rewrite(row['phases'][phase], **changes)
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)
        row = self.row('8.3.24')
        row['database']['empty_targets_proof'] = self.rewrite(
            row['database']['empty_targets_proof'], candidate_target_id='unrelated')
        with self.assertRaisesRegex(audit.EvidenceError, 'both fresh targets'):
            audit.check_row(row, self.root)

    def test_wrong_tool_commit_snapshot_or_failed_phase_is_rejected(self):
        for changes in ({'binary_sha256': 'd' * 64}, {'candidate_commit': 'd' * 40},
                        {'corpus_snapshot_sha256': 'd' * 64}, {'exit_code': 1}, {'exit_code': False}):
            row = self.row('8.3.25')
            phase = 'candidate_import_empty'
            row['phases'][phase] = self.rewrite(row['phases'][phase], **changes)
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)

    def test_full_scope_and_full_diff_counts_are_required(self):
        for changes in ({'scope': 'scoped'}, {'summary': {'left_only': 0, 'right_only': 0, 'different': 0, 'unchanged': 2}},
                        {'full_diff': self.ref({'summary': {}, 'differences': []})}):
            row = self.row('8.3.26')
            row['comparisons']['export'] = self.rewrite(row['comparisons']['export'], **changes)
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)

    def test_changed_source_shared_targets_and_missing_limits_are_rejected(self):
        for change in ('source', 'target', 'limits'):
            row = self.row('8.3.24')
            if change == 'source':
                row['database']['source_after'] = self.ref('changed')
            elif change == 'target':
                row['database']['candidate_target_id'] = 'native'
            else:
                del row['observations']['write_limits']
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)

    def test_missing_tampered_and_escaping_evidence_are_rejected(self):
        for ref in ({'path': '../outside', 'sha256': 'a' * 64},
                    {'path': '/outside', 'sha256': 'a' * 64},
                    {'path': 'missing', 'sha256': 'a' * 64},
                    {**self.ref('data'), 'sha256': 'a' * 64}):
            with self.assertRaises(audit.EvidenceError):
                audit.artifact(self.root, ref)

    def test_symlink_and_duplicate_keys_are_rejected(self):
        target = self.ref('data')
        link = self.root / f'link-{self.counter}'
        link.symlink_to(self.root / target['path'])
        with self.assertRaisesRegex(audit.EvidenceError, 'symlink'):
            audit.artifact(self.root, {**target, 'path': link.name})
        ref = self.ref('{"state":"blocked","state":"measured"}')
        with self.assertRaisesRegex(audit.EvidenceError, 'duplicate JSON key'):
            audit.load(self.root / ref['path'])

    def test_malformed_nested_objects_have_controlled_errors(self):
        for key in ('native', 'candidate', 'corpus', 'database', 'observations', 'phases', 'comparisons'):
            row = self.row('8.3.24')
            row[key] = None
            with self.assertRaises(audit.EvidenceError):
                audit.check_row(row, self.root)
        row = self.row('8.3.24')
        report_ref = row['comparisons']['export']
        report = audit.load(self.root / report_ref['path'])
        malformed = self.rewrite(report['full_diff'], differences=[None])
        row['comparisons']['export'] = self.rewrite(report_ref, full_diff=malformed)
        with self.assertRaisesRegex(audit.EvidenceError, 'invalid full diff entry'):
            audit.check_row(row, self.root)

    def test_blockers_remain_blocked_without_evidence(self):
        doc = {'schema_version': 1, 'rows': [{'release': release, 'state': 'blocked',
               'blockers': ['exact binary and SQL unavailable']} for release in audit.RELEASES]}
        self.assertEqual(audit.check(doc, self.root)['state'], 'blocked')
        doc['rows'][0]['blockers'] = []
        with self.assertRaises(audit.EvidenceError):
            audit.check(doc, self.root)

    def test_inventory_does_not_promote_directory_names_to_versions(self):
        root = self.root / f'inventory-{self.counter}'
        directory = root / '8.3.24.1819'
        directory.mkdir(parents=True)
        (directory / 'ibcmd').write_text('synthetic binary')
        result = audit.inventory(root)
        self.assertFalse(result['installed'][0]['runtime_version_verified'])
        self.assertEqual(result['native_acceptance'], 'not_measured')


if __name__ == '__main__':
    unittest.main()
