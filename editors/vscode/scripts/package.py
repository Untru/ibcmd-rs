"""Deterministic platform VSIX; explicit binary, retained staging, no publishing."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time
import zipfile


def validate_binary(binary, target):
    data = binary.read_bytes()
    if target == "win32-x64":
        if data[:2] != b"MZ" or len(data) < 64:
            raise ValueError("win32-x64 requires PE x64")
        offset = struct.unpack_from("<I", data, 60)[0]
        if data[offset:offset+6] != b"PE\0\0\x64\x86":
            raise ValueError("win32-x64 requires PE x64")
    elif data[:8] != b"\xcf\xfa\xed\xfe\x0c\0\0\x01":
        raise ValueError("darwin-arm64 requires arm64 Mach-O")


def normalize(source, output, epoch):
    timestamp = time.gmtime(max(315532800, epoch))[:6]
    with zipfile.ZipFile(source) as src, zipfile.ZipFile(output, "w") as dst:
        for name in sorted(src.namelist()):
            entry = zipfile.ZipInfo(name, timestamp)
            entry.create_system = 3
            entry.external_attr = (0o100755 if name.startswith("extension/bin/") else 0o100644) << 16
            dst.writestr(entry, src.read(name), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=["win32-x64", "darwin-arm64"], required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve(strict=True)
    validate_binary(binary, args.target)
    output = args.out.resolve()
    if output.exists():
        raise ValueError("Refusing to overwrite existing VSIX")
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH") or subprocess.check_output(["git", "log", "-1", "--format=%ct"], cwd=root, text=True).strip())
    stage = Path(tempfile.mkdtemp(prefix="ibcmd-vsix-"))
    for name in ["package.json", "README.md", "dist/extension.js"]:
        dest = stage / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / name, dest)
    shutil.copyfile(root.parents[1] / "LICENSE", stage / "LICENSE")
    (stage / "bin").mkdir()
    executable = "ibcmd-rs.exe" if args.target == "win32-x64" else "ibcmd-rs"
    shutil.copyfile(binary, stage / "bin" / executable)
    (stage / "bin" / executable).chmod(0o755)
    subprocess.run(["node", str(root / "node_modules/@vscode/vsce/vsce"), "package", "--no-dependencies", "--target", args.target, "--out", str(stage / "raw.vsix")], cwd=stage, check=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    normalize(stage / "raw.vsix", output, epoch)
    with zipfile.ZipFile(output) as package:
        assert {"extension/bin/" + executable, "extension/dist/extension.js", "extension/package.json"} <= set(package.namelist())
        assert json.loads(package.read("extension/package.json"))["main"] == "./dist/extension.js"
        assert package.read("extension/bin/" + executable) == binary.read_bytes()
        assert args.target in package.read("extension.vsixmanifest").decode()
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + ".sha256").write_text(f"{digest}  {output.name}\n")
    print(f"{output}\nSHA256 {digest}\nRetained staging: {stage}")


if __name__ == "__main__":
    main()
