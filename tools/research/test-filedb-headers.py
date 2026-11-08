"""Byte-anchor controls for the read-only native object-header inspector."""
import importlib.util
import io
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "header_inspector", Path(__file__).with_name("inspect-filedb-headers.py")
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def image(fat, length, raw12, pointers, pages=1024):
    data = bytearray(pages * 4096)
    struct.pack_into("<HHIIIQ", data, 2 * 4096, 0xFD1C, fat, 17, 23, raw12, length)
    for index, pointer in enumerate(pointers):
        struct.pack_into("<I", data, 2 * 4096 + 24 + index * 4, pointer)
    return data


class HeaderAnchors(unittest.TestCase):
    def test_existing_inventory_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "inventory.json"
            output.write_bytes(b"immutable evidence\n")
            arguments = ["inspect", "--database", directory, "--probe", "probe", "--output", str(output)]
            result = {"file": {}, "objects": {}, "rows": {}}
            with patch.object(sys, "argv", arguments), patch.object(module, "inspect", return_value=result):
                with self.assertRaises(FileExistsError):
                    module.main()
            self.assertEqual(output.read_bytes(), b"immutable evidence\n")

    def test_fat0_preserves_all_three_raw_words(self):
        data = image(0, 4096, 0xA1B2C3D4, [4])
        result = module.read_object(io.BytesIO(data), 4096, 1024, 2)
        self.assertEqual([result[key] for key in ("u32_4", "u32_8", "u32_12")],
                         [17, 23, 0xA1B2C3D4])
        self.assertEqual(result["required_fat_pages"], 0)

    def test_fat1_does_not_replace_raw12_with_derived_count(self):
        data = image(1, 1019 * 4096, 0, [3])
        struct.pack_into("<1019I", data, 3 * 4096, *range(4, 1023))
        result = module.read_object(io.BytesIO(data), 4096, 1024, 2)
        self.assertEqual(result["u32_12"], 0)
        self.assertEqual(result["required_fat_pages"], 1)
        self.assertEqual(result["used_data_pages"], 1019)

    def test_truncated_header_refused(self):
        with self.assertRaises(ValueError):
            module.read_object(io.BytesIO(bytes(2 * 4096 + 23)), 4096, 1024, 2)

    def test_invalid_fat_page_refused(self):
        data = image(1, 1019 * 4096, 0, [1024])
        with self.assertRaises(ValueError):
            module.read_object(io.BytesIO(data), 4096, 1024, 2)

    def test_impossible_length_refused_before_materialization(self):
        data = image(1, 2**64 - 1, 0, [3])
        with self.assertRaises(ValueError):
            module.read_object(io.BytesIO(data), 4096, 1024, 2)

    def test_repeated_data_page_refused(self):
        data = image(0, 2 * 4096, 0, [4, 4])
        with self.assertRaises(ValueError):
            module.read_object(io.BytesIO(data), 4096, 1024, 2)


if __name__ == "__main__":
    unittest.main()
