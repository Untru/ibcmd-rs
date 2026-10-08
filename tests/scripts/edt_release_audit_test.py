import importlib.util
import json
from pathlib import Path
import subprocess
import struct
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
    def elf_fixture(self):
        return bytearray((source.parent.parent / "tests/fixtures/elf-debug-numeric.bin").read_bytes())

    def test_numeric_debug_reference_is_distinct_from_loaded_bytes_and_strings(self):
        numeric = self.elf_fixture()
        self.assertIn(b".jar", numeric)
        self.assertEqual(audit.forbidden_binary_markers(numeric), [])
        for offset in (384, 416):
            data = self.elf_fixture()
            data[offset:offset + 4] = b".jar"
            self.assertIn(b".jar", audit.forbidden_binary_markers(data))

    def test_allocated_debug_info_is_scanned_and_load_segment_overlap_refuses(self):
        data = self.elf_fixture()
        struct.pack_into("<Q", data, 200, 2)
        self.assertIn(b".jar", audit.forbidden_binary_markers(data))
        data = self.elf_fixture()
        struct.pack_into("<Q", data, 592, 408)
        with self.assertRaisesRegex(SystemExit, "overlaps loadable"):
            audit.forbidden_binary_markers(data)

    def test_malformed_ranges_and_names_refuse_without_masking(self):
        for offset, value in ((216, 1 << 63), (320, (1 << 64) - 1)):
            data = self.elf_fixture()
            struct.pack_into("<Q", data, offset, value)
            with self.assertRaises(SystemExit):
                audit.forbidden_binary_markers(data)

    def test_extended_header_counts_use_the_actual_section_zero_fields(self):
        data = self.elf_fixture()
        for offset, value in ((60, 0), (62, 0xffff), (56, 0xffff)):
            struct.pack_into("<H", data, offset, value)
        struct.pack_into("<Q", data, 96, 5)
        struct.pack_into("<I", data, 104, 4)
        struct.pack_into("<I", data, 108, 1)
        self.assertEqual(audit.forbidden_binary_markers(data), [])

    def test_debug_cannot_hide_headers_string_table_or_other_section_payloads(self):
        for offset in (64, 416, 432, 560):
            data = self.elf_fixture()
            struct.pack_into("<Q", data, 216, offset)
            with self.assertRaises(SystemExit):
                audit.forbidden_binary_markers(data)

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
               "metadata": {"component": {"name": "ibcmd-rs", "licenses": [{"expression": "MIT"}]}},
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
            formatter_root = root / "crates/ibcmd-number-format"
            formatter_files = {"Cargo.toml", "LICENSE", "NOTICE.md"} | {
                path.relative_to(formatter_root).as_posix()
                for path in (formatter_root / "src").rglob("*.rs")
            }
            bundled_formatter_files = {
                name.split("/third-party/ibcmd-number-format/", 1)[1]
                for name in entries if "/third-party/ibcmd-number-format/" in name
            }
            self.assertEqual(bundled_formatter_files, formatter_files)
            for suffix in sorted(formatter_files):
                name = next(name for name in entries if name.endswith(
                    "/third-party/ibcmd-number-format/" + suffix))
                changed = directory / (suffix.replace("/", "-") + ".zip")
                with zipfile.ZipFile(changed, "w") as archive:
                    for key, data in sorted(entries.items()):
                        archive.writestr(key, data + b"changed" if key == name else data)
                with self.assertRaisesRegex(SystemExit, "source/notice differs"):
                    audit.audit_archive(changed, binary, bom)
                missing = directory / (suffix.replace("/", "-") + "-missing.zip")
                with zipfile.ZipFile(missing, "w") as archive:
                    for key, data in sorted(entries.items()):
                        if key != name:
                            archive.writestr(key, data)
                with self.assertRaisesRegex(SystemExit, "allowlist mismatch"):
                    audit.audit_archive(missing, binary, bom)
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


class ProjectLicenseDistribution(unittest.TestCase):
    def setUp(self):
        self.root = source.parent.parent
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.binary = self.directory / "ibcmd-rs.exe"
        self.binary.write_bytes(b"standalone license package fixture")
        generator_spec = importlib.util.spec_from_file_location(
            "generate_sbom", self.root / "scripts/generate_sbom.py")
        generator = importlib.util.module_from_spec(generator_spec)
        generator_spec.loader.exec_module(generator)
        root_package = tomllib.loads((self.root / "Cargo.toml").read_text(encoding="utf-8"))["package"]
        formatter_package = tomllib.loads((self.root / "crates/ibcmd-number-format/Cargo.toml").read_text(encoding="utf-8"))["package"]
        root_component = generator.component({**root_package, "id": "root"}, None, "root")
        formatter_component = generator.component({**formatter_package, "id": "formatter"}, None, "root")
        self.bom = {"bomFormat": "CycloneDX", "specVersion": "1.5",
                    "metadata": {"component": root_component},
                    "components": [root_component, formatter_component]}
        self.sbom = self.directory / "sbom.json"
        self.sbom.write_text(json.dumps(self.bom), encoding="utf-8")
        self.package = self.directory / "package.zip"
        subprocess.run([sys.executable, str(self.root / "scripts/package_release.py"),
                        "--binary", str(self.binary), "--sbom", str(self.sbom),
                        "--output", str(self.package), "--version", root_package["version"],
                        "--target", "x86_64-pc-windows-msvc", "--repository-root", str(self.root)],
                       check=True, capture_output=True)
        with zipfile.ZipFile(self.package) as archive:
            self.entries = {name: archive.read(name) for name in archive.namelist()}
        self.archive_root = next(iter(self.entries)).split("/", 1)[0]

    def rewritten_archive(self, entries):
        changed = self.directory / "changed.zip"
        with zipfile.ZipFile(changed, "w") as archive:
            for name, data in sorted(entries.items()):
                archive.writestr(name, data)
        return changed

    def test_package_and_generated_sbom_preserve_separate_licenses(self):
        self.assertEqual(self.bom["metadata"]["component"]["licenses"], [{"expression": "MIT"}])
        self.assertEqual(self.bom["components"][1]["licenses"],
                         [{"expression": "GPL-2.0-only WITH Classpath-exception-2.0"}])
        audit.audit_sbom(self.sbom)
        audit.audit_archive(self.package, self.binary, None)
        audit.audit_checksum(self.package, self.package.with_name("package.zip.sha256"))
        paths = {
            "LICENSE": "LICENSE",
            "third-party/ibcmd-number-format/LICENSE": "crates/ibcmd-number-format/LICENSE",
            "third-party/ibcmd-number-format/NOTICE.md": "crates/ibcmd-number-format/NOTICE.md",
            "third-party/morph1c/LICENSE-APACHE": "crates/ibcmd-edt/vendor/morph1c/LICENSE-APACHE",
            "third-party/morph1c/NOTICE.md": "crates/ibcmd-edt/vendor/morph1c/NOTICE.md",
        }
        for member, relative in paths.items():
            expected = (self.root / relative).read_text(encoding="utf-8").encode("utf-8")
            self.assertEqual(self.entries[f"{self.archive_root}/{member}"], expected)

    def test_missing_or_changed_root_license_is_rejected(self):
        name = f"{self.archive_root}/LICENSE"
        missing = {key: value for key, value in self.entries.items() if key != name}
        with self.assertRaisesRegex(SystemExit, "allowlist mismatch"):
            audit.audit_archive(self.rewritten_archive(missing), self.binary, None)
        changed = {**self.entries, name: self.entries[name].replace(b"2026", b"2025")}
        with self.assertRaisesRegex(SystemExit, "root MIT license differs"):
            audit.audit_archive(self.rewritten_archive(changed), self.binary, None)

    def test_missing_or_wrong_root_sbom_license_is_rejected_in_both_inputs(self):
        for licenses in (None, [{"expression": "Apache-2.0"}]):
            with self.subTest(licenses=licenses):
                bom = json.loads(json.dumps(self.bom))
                if licenses is None:
                    del bom["metadata"]["component"]["licenses"]
                else:
                    bom["metadata"]["component"]["licenses"] = licenses
                self.sbom.write_text(json.dumps(bom), encoding="utf-8")
                with self.assertRaisesRegex(SystemExit, "root license as MIT"):
                    audit.audit_sbom(self.sbom)
                changed = {**self.entries, f"{self.archive_root}/sbom.cdx.json": json.dumps(bom).encode("utf-8")}
                with self.assertRaisesRegex(SystemExit, "root license as MIT"):
                    audit.audit_archive(self.rewritten_archive(changed), self.binary, None)

    def test_conflicting_duplicate_root_sbom_license_is_rejected(self):
        self.bom["components"][0] = {**self.bom["components"][0], "licenses": [{"expression": "Apache-2.0"}]}
        self.sbom.write_text(json.dumps(self.bom), encoding="utf-8")
        with self.assertRaisesRegex(SystemExit, "root license as MIT"):
            audit.audit_sbom(self.sbom)


if __name__ == "__main__":
    unittest.main()
