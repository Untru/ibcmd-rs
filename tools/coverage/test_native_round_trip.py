"""Refusal isolation must retain declared same-class dependencies."""
import unittest

from native_round_trip import first_error, required_variants


class Dependencies(unittest.TestCase):
    def test_public_diagnostic_removes_both_path_spellings_before_truncation(self):
        root = 'F:/scratch/' + 'long-' * 80
        output = '[ERROR] ' + root.replace('/', '\\') + '/Form.xml: invalid enum'
        self.assertEqual(first_error(output, [root]), '[ERROR] <work>/Form.xml: invalid enum')

    def test_dependencies_are_explicit_and_known(self):
        self.assertEqual(required_variants({}, ['Template', 'One']), set())
        self.assertEqual(required_variants({'required_variants': ['One']}, ['Template', 'One']), {'One'})
        with self.assertRaises(SystemExit):
            required_variants({'required_variants': ['Missing']}, ['Template', 'One'])
        with self.assertRaises(SystemExit):
            required_variants({'required_variants': 'One'}, ['Template', 'One'])


if __name__ == '__main__':
    unittest.main()
