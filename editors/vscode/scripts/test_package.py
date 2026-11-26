import importlib.util
from pathlib import Path
import tempfile
import subprocess
import unittest
import zipfile
spec=importlib.util.spec_from_file_location("package",Path(__file__).with_name("package.py"))
package=importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)
class PackageTests(unittest.TestCase):
    def test_reproducibility(self):
        root=Path(tempfile.mkdtemp(prefix="ibcmd-vsix-test-"))
        for i,names in enumerate([["extension/z","extension/bin/ibcmd-rs.exe"],["extension/bin/ibcmd-rs.exe","extension/z"]]):
            with zipfile.ZipFile(root/f"raw{i}.zip","w") as archive:
                for name in names: archive.writestr(name,name.encode())
            package.normalize(root/f"raw{i}.zip",root/f"out{i}.zip",1700000000)
        self.assertEqual((root/"out0.zip").read_bytes(),(root/"out1.zip").read_bytes())
        with zipfile.ZipFile(root/"out0.zip") as archive:
            self.assertEqual(archive.getinfo("extension/bin/ibcmd-rs.exe").external_attr>>16 & 0o777,0o755)
    def test_windows_checkout_preserves_commonjs_lf(self):
        # Retain this snapshot; do not delete test output during teardown.
        root = Path(tempfile.mkdtemp(prefix="ibcmd-vsix-windows-checkout-"))
        repository = Path(__file__).resolve().parents[3]
        files = ["editors/vscode/scripts/build.cjs", "editors/vscode/scripts/python.cjs"]
        subprocess.run(
            ["git", "-c", "core.autocrlf=true", "checkout-index",
             "--prefix=" + root.as_posix() + "/", "--", *files],
            cwd=repository, check=True,
        )
        for name in files:
            data = (root / name).read_bytes()
            self.assertIn(b"\n", data)
            self.assertNotIn(b"\r\n", data, name)

    def test_wrong_platform(self):
        with self.assertRaisesRegex(ValueError,"PE x64"): package.validate_binary(Path(__file__),"win32-x64")
unittest.main()
