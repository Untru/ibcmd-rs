import importlib.util
from pathlib import Path
import unittest

source = Path(__file__).resolve().parents[2] / "scripts" / "audit_release.py"
spec = importlib.util.spec_from_file_location("audit_release", source)
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class MarkerBoundary(unittest.TestCase):
    def test_source_ids_are_accepted_but_do_not_mask_any_runtime_marker(self):
        project = b" ".join(audit.EDT_SOURCE_IDS)
        self.assertEqual(audit.forbidden_binary_markers(project), [])
        for marker in audit.FORBIDDEN_BINARY_MARKERS:
            payload = marker if marker != b"org.eclipse" else b"org.eclipse.equinox.launcher"
            self.assertIn(marker, audit.forbidden_binary_markers(project + b" " + payload))

    def test_arbitrary_eclipse_source_id_is_not_allowed(self):
        self.assertIn(b"org.eclipse", audit.forbidden_binary_markers(b"org.eclipse.unknown.builder"))

    def test_allowed_ids_cannot_mask_extended_package_or_class_names(self):
        for identifier in audit.EDT_SOURCE_IDS:
            if identifier.startswith(b".settings/"):
                continue
            for suffix in (b".evil.Launcher", b"/Runtime", b"$Runtime"):
                self.assertIn(b"org.eclipse", audit.forbidden_binary_markers(identifier + suffix))

    def test_source_preferences_path_is_distinct_from_runtime_package(self):
        self.assertEqual(audit.forbidden_binary_markers(b".settings/org.eclipse.core.resources.prefs.ibcmd-provenance/"), [])
        self.assertIn(b"org.eclipse", audit.forbidden_binary_markers(b"org.eclipse.core.resources.prefs.evil.Launcher"))


if __name__ == "__main__":
    unittest.main()
