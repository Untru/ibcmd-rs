import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch
import fixture_paths as f


class FixtureContract(unittest.TestCase):
    def test_actual_pinned_checkout(self):
        f.validate()

    def test_missing_checkout_is_an_error(self):
        with tempfile.TemporaryDirectory() as td:
            with self.assertRaisesRegex(ValueError, 'missing'):
                f.check_identity(pathlib.Path(td))

    def identity_case(self, stage, head='a' * 40):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            module = f.fixture_root(root)
            module.mkdir(parents=True)
            (module / '.git').write_text('gitdir: unused', encoding='utf-8')
            with patch.object(f, 'git', side_effect=[stage, head, f.URL, '']):
                f.check_identity(root)

    def test_non_gitlink_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'pinned'):
            self.identity_case('100644 ' + 'a' * 40 + ' 0\t' + f.SUBMODULE)

    def test_wrong_revision_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'HEAD differs'):
            self.identity_case('160000 ' + 'a' * 40 + ' 0\t' + f.SUBMODULE, 'b' * 40)

    def baseline_case(self, mutate):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            module = f.fixture_root(root)
            def put(path, data):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(data), encoding='utf-8')
            base = root/'tests/fixtures/coverage/build/set'
            native = module/'coverage/build/set'
            put(base/'baseline.json', {'equal': ['CovOne']})
            put(native/'manifest.json', {'variants': [{'name': 'CovOne', 'status': 'platform_proven'}]})
            put(module/'coverage-records/source-probes.json', {'cases': [{'id': 'one'}]})
            put(root/'tests/fixtures/coverage-records/source-probe-expectations.json', {'cases': {'one': {'expected_failure': 'precise'}}})
            mutate(base, native, put)
            return f.check_baselines(root, module)

    def test_existing_expectations_are_kept(self):
        self.assertEqual(self.baseline_case(lambda *_: None), (1, 1, 1))

    def test_identity_interface_can_require_native_parity(self):
        def change(base, native, put):
            root = base.parents[4]
            module = f.fixture_root(root)
            put(module/'coverage-records/source-probes.json', {'cases': [{'id': 'one', 'native_behavior': 'native_cf', 'operation': {'kind': 'identity'}, 'target': 'Interfaces/Probe.xml'}]})
            put(root/'tests/fixtures/coverage-records/source-probe-expectations.json', {'cases': {'one': {'expected_parity': 'native_interface_descriptor'}}})
        self.assertEqual(self.baseline_case(change), (1, 1, 1))

    def test_mutated_source_cannot_be_relabeled_as_native_parity(self):
        def change(base, native, put):
            root = base.parents[4]
            put(root/'tests/fixtures/coverage-records/source-probe-expectations.json', {'cases': {'one': {'expected_parity': 'native_interface_descriptor'}}})
        with self.assertRaisesRegex(ValueError, 'unsupported parity'):
            self.baseline_case(change)

    def test_ambiguous_refusal_and_parity_is_rejected(self):
        def change(base, native, put):
            root = base.parents[4]
            put(root/'tests/fixtures/coverage-records/source-probe-expectations.json', {'cases': {'one': {'expected_failure': 'precise', 'expected_parity': 'native_interface_descriptor'}}})
        with self.assertRaisesRegex(ValueError, 'exactly one'):
            self.baseline_case(change)

    def test_removed_set_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'orphaned baseline set'):
            self.baseline_case(lambda base, native, put: (native/'manifest.json').unlink())

    def test_removed_variant_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'orphaned baseline entries'):
            self.baseline_case(lambda base, native, put: put(native/'manifest.json', {'variants': []}))

    def test_removed_storage_path_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'orphaned baseline entries'):
            self.baseline_case(lambda base, native, put: put(base/'storage-baseline.json', {'equal': ['missing.xml']}))

    def test_missing_refusal_is_rejected(self):
        def change(base, native, put):
            root = base.parents[4]
            put(root/'tests/fixtures/coverage-records/source-probe-expectations.json', {'cases': {}})
        with self.assertRaisesRegex(ValueError, 'probe IDs'):
            self.baseline_case(change)


if __name__ == '__main__':
    unittest.main()
