"""Compare every relative file path and byte in two completed PG exports."""

import argparse
import hashlib
import json
from pathlib import Path


def files(root):
    if not root.is_dir():
        raise NotADirectoryError(root)
    found = {}

    def raise_error(error):
        raise error

    for directory, _, names in root.walk(on_error=raise_error):
        for name in names:
            path = directory / name
            found[path.relative_to(root).as_posix()] = path
    return found


def record(tree, name, digest):
    tree.update(name.encode("utf-8") + b"\0" + digest)


def compare(native, candidate):
    left, right = files(native), files(candidate)
    if not {"Configuration.xml", "ConfigDumpInfo.xml"}.issubset(left):
        raise ValueError(f"Incomplete native export at {native}")
    trees = [hashlib.sha256(), hashlib.sha256()]
    counts = dict(identical=0, different=0, native_only=0, candidate_only=0)
    for name in sorted(left.keys() | right.keys()):
        if name in left and name in right:
            digests = [hashlib.sha256(), hashlib.sha256()]
            identical = True
            with left[name].open("rb") as a, right[name].open("rb") as b:
                while True:
                    x, y = a.read(1 << 20), b.read(1 << 20)
                    if not x and not y:
                        break
                    identical &= x == y
                    digests[0].update(x)
                    digests[1].update(y)
            for tree, digest in zip(trees, digests):
                record(tree, name, digest.digest())
            counts["identical" if identical else "different"] += 1
        else:
            side = 0 if name in left else 1
            path = (left, right)[side][name]
            with path.open("rb") as stream:
                record(trees[side], name, hashlib.file_digest(stream, "sha256").digest())
            counts[("native_only", "candidate_only")[side]] += 1
    return dict(**counts, native_tree=trees[0].hexdigest(), candidate_tree=trees[1].hexdigest())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    result = compare(args.native, args.candidate)
    print(json.dumps(result, indent=2))
    raise SystemExit(int(any(result[k] for k in ("different", "native_only", "candidate_only"))))
