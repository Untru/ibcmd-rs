"""Retained clean-room Git checkout byte controls; no Windows proof."""
from pathlib import Path
import shutil
import subprocess
import unittest

import rpc_probe as probe

RULES = ("scripts/status-lab/** text eol=lf", "src/server/mod.rs text eol=lf",
         "src/server/rpc.rs text eol=lf", "src/stored_objects.rs text eol=lf")
PATHS = ("scripts/status-lab/control.py", "scripts/status-lab/control.json",
         "scripts/status-lab/control.md", "src/server/mod.rs", "src/server/rpc.rs", "src/stored_objects.rs")


class CheckoutBytes(unittest.TestCase):
    def checkout(self, include_rules):
        root = probe.create_fixture_root().root
        repo = root / "public-git-fixture"; repo.mkdir()
        git = shutil.which("git")
        self.assertIsNotNone(git, "Git required; never silently skip byte controls")
        def run(*args):
            return subprocess.run([git, "-C", str(repo), *args], check=True,
                capture_output=True, timeout=5).stdout
        attributes = Path(__file__).resolve().parents[2].joinpath(".gitattributes").read_text()
        self.assertTrue(all(rule in attributes.splitlines() for rule in RULES))
        if not include_rules:
            attributes = "\n".join(line for line in attributes.splitlines() if line not in RULES) + "\n"
        (repo / ".gitattributes").write_text(attributes)
        data = b"public first line\npublic second line\n"
        for relative in PATHS:
            path = repo / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
        run("-c", "init.defaultBranch=fixture", "init", "--quiet")
        run("-c", "core.autocrlf=false", "add", "--", ".gitattributes", *PATHS)
        for relative in PATHS:
            saved = root / "saved" / relative; saved.parent.mkdir(parents=True, exist_ok=True)
            (repo / relative).rename(saved)
        # Fresh checkout under the Windows-style conversion setting. No originals
        # deleted, global settings changed, or cached-index restore assumption.
        run("-c", "core.autocrlf=true", "checkout-index", "--all", "--force")
        return [(repo / relative).read_bytes() for relative in PATHS], data
    def test_receipt_sources_checkout_as_exact_lf_even_with_autocrlf(self):
        actual, expected = self.checkout(True)
        self.assertEqual(actual, [expected] * len(PATHS))
    def test_without_scoped_rules_checkout_changes_receipt_bytes(self):
        actual, expected = self.checkout(False)
        self.assertEqual(actual, [expected.replace(b"\n", b"\r\n")] * len(PATHS))


if __name__ == "__main__":
    unittest.main()
