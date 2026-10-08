"""Exercise version requirements independently of the release runner's libc."""

import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location(
    "audit_linux_abi", Path(__file__).resolve().parents[2] / "scripts/audit_linux_abi.py")
abi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(abi)


def needs(*names):
    return "Version needs section '.gnu.version_r' contains 1 entry:\n" + "\n".join(
        f"  0x0010: Name: {name} Flags: none Version: 2" for name in names)


class LinuxAbiTests(unittest.TestCase):
    def test_ubuntu_22_baseline_and_numeric_version_order(self):
        self.assertEqual(abi.glibc_requirements(needs("GLIBC_2.9", "GLIBC_2.35", "GLIBC_2.9")),
                         [(2, 9), (2, 35)])

    def test_reported_release_failure_is_refused(self):
        for name in ("GLIBC_2.38", "GLIBC_2.39", "GLIBC_2.35.1"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "exceeds"):
                abi.glibc_requirements(needs("GLIBC_2.17", name))

    def test_private_and_malformed_versions_are_refused(self):
        for name in ("GLIBC_PRIVATE", "GLIBC_2.bad"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "unsupported"):
                abi.glibc_requirements(needs(name))

    def test_symbol_and_definition_names_do_not_count_as_requirements(self):
        data = "Version symbols section '.gnu.version':\n  GLIBC_2.99\n"
        data += needs("GLIBC_2.17", "GLIBCXX_3.4.99")
        data += "\nVersion definition section '.gnu.version_d':\n Name: GLIBC_2.99\n"
        self.assertEqual(abi.glibc_requirements(data), [(2, 17)])

    def test_missing_requirements_are_refused(self):
        for data in ("", "No version information found in this file.", needs("GLIBCXX_3.4")):
            with self.subTest(data=data), self.assertRaisesRegex(ValueError, "no GLIBC"):
                abi.glibc_requirements(data)

    def test_actual_target_machine_headers(self):
        for machine in (62, 183):
            header = bytearray(64)
            header[:7] = b"\x7fELF\x02\x01\x01"
            struct.pack_into("<H", header, 18, machine)
            self.assertEqual(abi.elf_machine(header), machine)

    def test_wrong_class_endianness_and_truncation_are_refused(self):
        for header in (b"MZ" + bytes(62), b"\x7fELF\x01\x01\x01" + bytes(57),
                       b"\x7fELF\x02\x02\x01" + bytes(57), b"\x7fELF\x02\x01\x01"):
            with self.subTest(header=header), self.assertRaises(ValueError):
                abi.elf_machine(header)


if __name__ == "__main__":
    unittest.main()
