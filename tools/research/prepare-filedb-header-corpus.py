"""Create deterministic disposable input variants from the tracked tiny fixture."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import shutil
import zlib


def inventory(root):
    files = []
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*"), key=lambda p: p.relative_to(root).as_posix()):
        if path.is_file():
            name = path.relative_to(root).as_posix()
            value = path.read_bytes()
            value_hash = hashlib.sha256(value).hexdigest()
            digest.update(name.encode("utf-8") + b"\0" + value_hash.encode("ascii") + b"\n")
            files.append({"path": name, "bytes": len(value), "sha256": value_hash})
    return {"files": files, "tree_sha256": digest.hexdigest()}


def large_module(original, mebibytes):
    return module_from_bytes(original, mebibytes * 1024 * 1024)


def module_from_bytes(original, byte_count):
    random_bytes = hashlib.shake_256(b"ibcmd-rs issue525 native-header-corpus v1").digest(byte_count)
    text = base64.b64encode(random_bytes)
    comments = b"\r\n" + b"".join(b"// " + text[i:i + 120] + b"\r\n" for i in range(0, len(text), 120))
    return original + comments


def prepare(fixture, output, boundary_bytes=None):
    if output.exists():
        raise ValueError("Output already exists; a fresh directory is required")
    source = inventory(fixture)
    modules = list(fixture.rglob("Module.bsl"))
    if len(modules) != 1:
        raise ValueError("Expected the single-module tiny fixture")
    relative = modules[0].relative_to(fixture)
    original = modules[0].read_bytes()
    old, new = "Возврат 42;".encode("utf-8"), "Возврат 43;".encode("utf-8")
    if original.count(old) != 1:
        raise ValueError("Unexpected tiny fixture function")
    replacement = original.replace(old, new)
    if len(replacement) != len(original):
        raise ValueError("The small rewrite changes source length")
    large = large_module(original, 24)
    compressor = zlib.compressobj(level=6, wbits=-15)
    estimated = len(compressor.compress(large) + compressor.flush())
    if estimated < 22 * 1024 * 1024:
        raise ValueError("Corpus is insufficient for the intended FAT1 measurement")
    variants = {}
    inputs = [("small", original), ("same_length", replacement), ("large", large),
              ("medium_growth", large_module(original, 36)),
              ("extra_growth", large_module(original, 48))]
    if boundary_bytes is not None:
        if not 0 < boundary_bytes <= 48 * 1024 * 1024:
            raise ValueError("Boundary payload is outside the measured generator budget")
        inputs.append(("boundary", module_from_bytes(original, boundary_bytes)))
    for label, module in inputs:
        destination = output / label
        shutil.copytree(fixture, destination)
        (destination / relative).write_bytes(module)
        variants[label] = inventory(destination)
    if inventory(fixture) != source:
        raise ValueError("Source fixture changed")
    result = {"schema_version": 1, "fixture": source, "variants": variants,
              "large_module_bytes": len(large), "large_raw_deflate_estimate": estimated,
              "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "clean_room": True, "native_FAT1_not_yet_confirmed": True,
              "boundary_random_bytes": boundary_bytes}
    (output / "manifest.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--boundary-bytes", type=int)
    args = parser.parse_args()
    result = prepare(args.fixture, args.output, args.boundary_bytes)
    print(json.dumps({key: result[key] for key in ("large_module_bytes", "large_raw_deflate_estimate", "clean_room")}))
