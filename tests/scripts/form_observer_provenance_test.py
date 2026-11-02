"""Retained public staging controls; no real executable or private input runs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import uuid

if os.environ.get("IBCMD_TEST_KEEP_FIXTURES") != "1":
    raise SystemExit("IBCMD_TEST_KEEP_FIXTURES=1 is required")
if len(sys.argv) != 2:
    raise SystemExit("Usage: form_observer_provenance_test.py RETAINED_FIXTURE_PARENT")
PARENT = Path(sys.argv.pop()).resolve()
PARENT.mkdir(parents=True, exist_ok=True)
WRITER = Path(__file__).resolve().parents[2] / "tools/write-form-observer-provenance.py"


class ProvenanceStagingTests(unittest.TestCase):
    def setUp(self):
        self.case = PARENT / ("case-" + uuid.uuid4().hex)
        self.case.mkdir()
        self.repo = self.case / "source"
        self.repo.mkdir()
        self.env = dict(os.environ)
        for key in tuple(self.env):
            if key.startswith("GIT_CONFIG_") or key in (
                "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR",
                "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            ):
                del self.env[key]
        self.env.update(
            GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
            GIT_AUTHOR_NAME="PublicFixture", GIT_COMMITTER_NAME="PublicFixture",
            GIT_AUTHOR_EMAIL="fixture@example.invalid", GIT_COMMITTER_EMAIL="fixture@example.invalid",
            GIT_AUTHOR_DATE="2001-01-01T00:00:00+00:00", GIT_COMMITTER_DATE="2001-01-01T00:00:00+00:00",
        )
        hooks = self.case / "empty-hooks"
        hooks.mkdir()
        self.git_args = ["git", "-c", "core.hooksPath=" + str(hooks), "-c", "commit.gpgsign=false"]
        self.git("init", "--quiet")
        (self.repo / ".gitignore").write_text("/target/\n")
        (self.repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.95.0"\n')
        (self.repo / "source.txt").write_text("public synthetic source\n")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "public fixture")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.env["DIAGNOSTIC_REQUESTED_HEAD"] = self.commit
        self.binary = self.case / "public-placeholder.exe"
        self.binary.write_bytes(b"public staging bytes; never executed\n")
        self.cached = self.repo / "target/form-observer-artifact"
        self.cached.mkdir(parents=True)
        (self.cached / "provenance.json").write_bytes(b"retained old provenance\n")
        (self.cached / "old.exe").write_bytes(b"retained old public bytes\n")

    def git(self, *args):
        return subprocess.check_output(
            self.git_args + list(args), cwd=self.repo, env=self.env,
            text=True, stderr=subprocess.PIPE, timeout=30,
        )

    @staticmethod
    def snapshot(path):
        return {p.name: p.read_bytes() for p in path.iterdir() if p.is_file()}

    def stage(self, output, label):
        result = subprocess.run(
            [sys.executable, str(WRITER), str(self.binary), str(output)],
            cwd=self.repo, env=self.env, text=True, capture_output=True, timeout=30,
        )
        (self.case / (label + ".stdout")).write_text(result.stdout)
        (self.case / (label + ".stderr")).write_text(result.stderr)
        return result

    def fresh(self):
        return self.case / "runner-temp/form-observer-artifact-public-run-attempt1"

    def test_cached_target_is_refused_without_mutating_prior_artifacts(self):
        before = self.snapshot(self.cached)
        result = self.stage(self.cached, "cached-refusal")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("FileExistsError", result.stderr)
        self.assertEqual(self.snapshot(self.cached), before)

    def test_fresh_runner_output_binds_current_bytes_and_source(self):
        before = self.snapshot(self.cached)
        output = self.fresh()
        result = self.stage(output, "fresh-success")
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((output / "provenance.json").read_text())
        self.assertEqual(manifest["builtCommit"], self.commit)
        self.assertEqual(manifest["requestedHead"], self.commit)
        self.assertEqual(manifest["binary"], self.binary.name)
        self.assertEqual(manifest["binaryBytes"], self.binary.stat().st_size)
        self.assertEqual(manifest["binarySHA256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual((output / self.binary.name).read_bytes(), self.binary.read_bytes())
        self.assertIs(manifest["privateInputUsed"], False)
        self.assertEqual(self.snapshot(self.cached), before)
        self.assertEqual(set(self.snapshot(output)), {"provenance.json", self.binary.name})

    def test_duplicate_fresh_output_keeps_completed_receipt_and_binary(self):
        output = self.fresh()
        first = self.stage(output, "first-success")
        self.assertEqual(first.returncode, 0, first.stderr)
        before = self.snapshot(output)
        second = self.stage(output, "duplicate-refusal")
        self.assertNotEqual(second.returncode, 0)
        self.assertEqual(self.snapshot(output), before)

    def test_dirty_source_refuses_before_any_output_is_created(self):
        (self.repo / "source.txt").write_text("modified public source\n")
        output = self.fresh()
        result = self.stage(output, "dirty-refusal")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Source checkout is not clean", result.stderr)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
