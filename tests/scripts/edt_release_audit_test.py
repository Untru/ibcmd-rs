import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
import zipfile

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


class FormatterDistribution(unittest.TestCase):
    def test_package_contains_complete_current_formatter_source_and_rejects_tampering(self):
        root = source.parent.parent
        version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
        bom = {"bomFormat": "CycloneDX", "specVersion": "1.5",
               "metadata": {"component": {"name": "ibcmd-rs"}},
               "components": [{"name": "ibcmd-number-format", "licenses": [
                   {"expression": "GPL-2.0-only WITH Classpath-exception-2.0"}]}]}
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            binary = directory / "ibcmd-rs.exe"
            binary.write_bytes(b"standalone package fixture")
            sbom = directory / "sbom.json"
            sbom.write_text(json.dumps(bom), encoding="utf-8")
            package = directory / "package.zip"
            subprocess.run([sys.executable, str(root / "scripts/package_release.py"),
                            "--binary", str(binary), "--sbom", str(sbom),
                            "--output", str(package), "--version", version,
                            "--target", "x86_64-pc-windows-msvc", "--repository-root", str(root)],
                           check=True, capture_output=True)
            audit.audit_sbom(sbom)
            audit.audit_archive(package, binary, bom)
            audit.audit_checksum(package, package.with_name("package.zip.sha256"))
            with zipfile.ZipFile(package) as archive:
                entries = {name: archive.read(name) for name in archive.namelist()}
            for suffix in ("Cargo.toml", "src/lib.rs", "LICENSE", "NOTICE.md"):
                name = next(name for name in entries if name.endswith(
                    "/third-party/ibcmd-number-format/" + suffix))
                changed = directory / (suffix.replace("/", "-") + ".zip")
                with zipfile.ZipFile(changed, "w") as archive:
                    for key, data in sorted(entries.items()):
                        archive.writestr(key, data + b"changed" if key == name else data)
                with self.assertRaisesRegex(SystemExit, "source/notice differs"):
                    audit.audit_archive(changed, binary, bom)
            extra = directory / "extra.zip"
            with zipfile.ZipFile(extra, "w") as archive:
                for key, data in sorted({**entries, f"ibcmd-rs-{version}-x86_64-pc-windows-msvc/third-party/extra.rs": b"unknown"}.items()):
                    archive.writestr(key, data)
            with self.assertRaisesRegex(SystemExit, "allowlist mismatch"):
                audit.audit_archive(extra, binary, bom)
            bom["components"][0]["licenses"][0]["expression"] = "Apache-2.0"
            sbom.write_text(json.dumps(bom), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "actual license"):
                audit.audit_sbom(sbom)


if __name__ == "__main__":
    unittest.main()
