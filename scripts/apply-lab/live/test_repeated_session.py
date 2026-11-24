import unittest

from repeated_session import summarize


def journal():
    rows = ["1|probe|open|7|||режим=poll"]
    for tick, marker in [(2, "A"), (12, "A"), (13, "B"), (22, "B"), (23, "C")]:
        rows.append(f"{tick}|probe|poll|7|{marker}|{marker}|sid-source=marker-call draft")
    return "\n".join(rows)


def measure(rows):
    return summarize(rows.encode("utf-8-sig"), 5, 10, 15, 20)


class RepeatedSessionTest(unittest.TestCase):
    def test_only_complete_observations_in_each_interval(self):
        counts = measure(journal())["observations"]
        self.assertEqual(counts, {
            key: [{"client": value, "server": value, "samples": 1}]
            for key, value in zip(("before", "between", "after"), "ABC")
        })

    def test_existing_client_can_stay_on_old_generation(self):
        report = measure(journal().replace("|C|C|", "|A|A|"))
        self.assertEqual(report["observations"]["after"][0]["server"], "A")

    def test_rejects_invalid_evidence(self):
        for rows in (
            journal().replace("|poll|7|C|", "|poll|8|C|"),
            journal().replace("sid-source=marker-call ", ""),
            journal().replace("|A|A|", "|A|ОШИБКА|"),
            journal().replace("13|probe", "9|probe"),
            journal().split("\n23|")[0],
            journal() + "\n1|probe|open|7|||режим=poll",
        ):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                measure(rows)
