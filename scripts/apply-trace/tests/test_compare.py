"""Offline tests of compare_traces.py.  Run: python -m unittest discover -s scripts/apply-trace/tests"""

import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import compare_traces as ct  # noqa: E402

HEADER = "seq\tt_s\tspid\ttx\top\ttable\tname\tname2\tpartno\trows\tsize\tpayload_bytes\tpayload_sha256\tgroup\tsrc\tnote\n"


def writes(*rows):
    """rows: (op, table, name, name2, src, rows, payload_bytes)"""
    lines = [HEADER]
    for i, (op, table, name, name2, src, n, size) in enumerate(rows, 1):
        lines.append("\t".join([str(i), "0", "1", "0", op, table, name, name2, "0", str(n), "", str(size), "", "", src, ""]) + "\n")
    return "".join(lines)


class Families(unittest.TestCase):
    def _write(self, text):
        d = tempfile.mkdtemp()
        with open(os.path.join(d, "service-writes.tsv"), "w", encoding="utf-8", newline="") as fh:
            fh.write(text)
        return d

    def test_family_shapes(self):
        g = "8c2ac6ff-7309-4025-93f6-264cbb068d62"
        rows = [
            {"op": "UPDATE", "table": "Config", "name": f"{g}.new", "name2": g, "src": ""},
            {"op": "INSERT", "table": "Config", "name": "root.new", "name2": "", "src": "ConfigSave:root"},
            {"op": "DELETE", "table": "ConfigSave", "name": "LIKE %.new", "name2": "", "src": ""},
            {"op": "DELETE", "table": "_ConfigChngRNG", "name": "", "name2": "", "src": ""},
        ]
        self.assertEqual(ct.family(rows[0]), ("Config", "UPDATE", "<guid>.new -> <guid>"))
        self.assertEqual(ct.family(rows[1]), ("Config", "INSERT", "root.new <- ConfigSave"))
        self.assertEqual(ct.family(rows[2]), ("ConfigSave", "DELETE", "LIKE %.new"))
        self.assertEqual(ct.family(rows[3]), ("_ConfigChngRNG", "DELETE", ""))

    def test_load_counts_statements_rows_and_bytes(self):
        d = self._write(writes(
            ("INSERT", "Params", "DBNames.New", "", "", 1, 0),
            ("UPDATE", "Params", "DBNames.New", "", "", 1, 100),
            ("UPDATE", "Params", "DBNames.New", "", "", 1, 50),
        ))
        order, cells = ct.load(d)
        self.assertEqual(order, [("Params", "INSERT", "DBNames.New"), ("Params", "UPDATE", "DBNames.New")])
        self.assertEqual(cells[("Params", "UPDATE", "DBNames.New")], [2, 2, 150])
        self.assertEqual(ct.cell_text(cells[("Params", "UPDATE", "DBNames.New")]), "2 st / 2 rows / 150 B")
        self.assertEqual(ct.cell_text(None), "")

    def test_merge_order_puts_new_families_after_their_predecessor(self):
        a, b, c, d = ("T", "X", "a"), ("T", "X", "b"), ("T", "X", "c"), ("T", "X", "d")
        self.assertEqual(ct.merge_order([[a, c], [a, b, c, d]]), [a, b, c, d])
        # a family that opens its run and is unknown so far goes to the end
        self.assertEqual(ct.merge_order([[a, b], [d, a]]), [a, b, d])

    def test_human(self):
        self.assertEqual(ct.human(999), "999 B")
        self.assertEqual(ct.human(2048), "2.0 KB")
        self.assertEqual(ct.human(3 * 1024 * 1024), "3.0 MB")


if __name__ == "__main__":
    unittest.main()
