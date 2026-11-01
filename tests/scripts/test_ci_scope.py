"""Lane selection of the offline matrix (scripts/ci_scope.py)."""
import importlib.util
from pathlib import Path
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/ci_scope.py"
spec = importlib.util.spec_from_file_location("ci_scope", SCRIPT)
ci_scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci_scope)

ALL = dict(rust=True, crates=True, lib=True)


class ScopeTests(unittest.TestCase):
    def scope(self, files, referenced=()):
        with mock.patch.object(ci_scope, "referenced", lambda path: path in referenced):
            return ci_scope.scope(files)

    def test_unreferenced_documents_and_the_editor_skip_rust(self):
        self.assertEqual(
            self.scope(["README.md", "docs/guide.md", "editors/vscode/src/a.ts"]),
            dict(rust=False, crates=False, lib=False),
        )

    def test_a_document_a_test_reads_runs_everything(self):
        self.assertEqual(self.scope(["docs/HISTORY.md"], {"docs/HISTORY.md"}), ALL)

    def test_root_source_skips_the_crates(self):
        self.assertEqual(
            self.scope(["src/mcp.rs", "tests/cf_load.rs", "README.md"]),
            dict(rust=True, crates=False, lib=True),
        )

    def test_integration_suites_skip_the_crates_and_the_library(self):
        self.assertEqual(
            self.scope(["tests/cf_load.rs", "tests/support/elf_debug.rs", "tests/scripts/x.py"]),
            dict(rust=True, crates=False, lib=False),
        )

    def test_shared_test_code_the_library_includes_runs_the_library(self):
        self.assertEqual(
            self.scope(["tests/common/mcp_export_fixture.rs"]),
            dict(rust=True, crates=False, lib=True),
        )

    def test_fixtures_profiles_and_manifests_run_everything(self):
        for path in ["tests/fixtures/cf/a.b64", "profiles/xml/2.20.json", "Cargo.lock",
                     "crates/ibcmd-xml/src/lib.rs", ".github/workflows/ci.yml",
                     "rust-toolchain.toml", "xtask/src/main.rs", "docs/evidence/a.json"]:
            self.assertEqual(self.scope([path]), ALL, path)

    def test_nested_test_directories_are_not_suites(self):
        self.assertEqual(self.scope(["tests/fixtures/x.rs"]), ALL)

    def test_no_change_runs_nothing(self):
        self.assertEqual(self.scope([]), dict(rust=False, crates=False, lib=False))

    def lanes(self, files, pr_release=False, referenced=()):
        with mock.patch.object(ci_scope, "referenced", lambda path: path in referenced):
            return ci_scope.lanes(files, pr_release)

    def test_a_pull_request_builds_the_release_binary_only_where_asked(self):
        self.assertTrue(self.lanes(["src/mcp.rs"], pr_release=True)["release"])
        self.assertFalse(self.lanes(["src/mcp.rs"], pr_release=False)["release"])

    def test_no_release_build_without_the_rust_lane(self):
        self.assertFalse(self.lanes(["README.md"], pr_release=True)["release"])
        self.assertFalse(self.lanes([], pr_release=True)["release"])

    def test_windows_powershell_runs_when_its_inputs_change(self):
        for path in ["tools/validate-edt-corpus.ps1", "tests/scripts/validate-edt-corpus.Tests.ps1",
                     "crates/ibcmd-schema/data/edt-x.json", ".github/workflows/offline-e2e.yml"]:
            self.assertTrue(self.lanes(["src/mcp.rs", path])["powershell"], path)

    def test_windows_powershell_skips_other_changes(self):
        for path in ["src/mcp.rs", "tools/validate-physical-adapter-policy.ps1",
                     "crates/ibcmd-schema/src/lib.rs", ".github/workflows/ci.yml", "README.md"]:
            self.assertFalse(self.lanes([path])["powershell"], path)

    def test_pushes_and_manual_runs_run_every_lane(self):
        for argv in (["ci_scope.py", "--all"], ["ci_scope.py"], ["ci_scope.py", "--all", "--base", "x"]):
            with mock.patch("sys.argv", argv), mock.patch("builtins.print") as printed:
                ci_scope.main()
            lines = [call.args[0] for call in printed.call_args_list]
            self.assertEqual(
                lines,
                ["rust=true", "crates=true", "lib=true", "release=true", "powershell=true"],
                argv,
            )

    def test_a_pull_request_passes_its_matrix_entry_to_the_release_lane(self):
        for flag, release in (([], "release=false"), (["--pr-release"], "release=true")):
            argv = ["ci_scope.py", "--base", "origin/master", *flag]
            with mock.patch("sys.argv", argv), \
                    mock.patch.object(ci_scope, "changed_files", lambda base: ["src/mcp.rs"]), \
                    mock.patch("builtins.print") as printed:
                ci_scope.main()
            lines = [call.args[0] for call in printed.call_args_list if not call.kwargs]
            self.assertEqual(
                lines, ["rust=true", "crates=false", "lib=true", release, "powershell=false"], argv
            )

    def test_reference_lookup_finds_a_cited_document(self):
        # profiles cite evidence documents; a real lookup against the tree.
        self.assertTrue(ci_scope.referenced("docs/HISTORY.md"))
        # Built at run time: a literal path here would be a reference itself.
        missing = "docs/" + "no-such-document-for-ci-scope.md"
        self.assertFalse(ci_scope.referenced(missing))


if __name__ == "__main__":
    unittest.main()
