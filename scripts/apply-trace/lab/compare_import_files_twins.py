"""Check native XML exports of the two_variable fixture after the #363 edits."""

import argparse
import hashlib
from pathlib import Path
import re


MODULE = "CommonForms/ФормаСлева/Ext/Form/Module.bsl"
DESCRIPTOR = "CommonForms/ФормаСправа.xml"
VERSIONS = "ConfigDumpInfo.xml"


def files(root):
    return {
        file.relative_to(root).as_posix(): file.read_bytes()
        for file in root.rglob("*")
        if file.is_file()
    }


def replace_once(data, old, new):
    before = old.encode()
    if data.count(before) != 1:
        raise ValueError(f"Expected one occurrence of {old!r}")
    return data.replace(before, new.encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("exports", type=Path, nargs="+")
    args = parser.parse_args()
    expected = files(args.baseline)
    expected[MODULE] = replace_once(expected[MODULE], "Номер = 1;", "Номер = 2;")
    expected[DESCRIPTOR] = replace_once(
        expected[DESCRIPTOR], "<Comment/>", "<Comment>Изменено частичной загрузкой</Comment>"
    )
    # Keep object names, IDs, ordering and every byte outside version values.
    version = re.compile(rb'configVersion="[0-9a-fA-F]{40}"')
    if not version.search(expected[VERSIONS]):
        raise ValueError("Missing configVersion entries in baseline")
    for root in args.exports:
        actual = files(root)
        different = sorted(expected.keys() ^ actual.keys())
        for path in expected.keys() & actual.keys():
            left, right = expected[path], actual[path]
            if path == VERSIONS:
                left, right = version.sub(b'configVersion=""', left), version.sub(b'configVersion=""', right)
            if left != right:
                different.append(path)
        if different:
            raise SystemExit(f"{root}: unexpected differences: {sorted(different)}")
        digest = hashlib.sha256(actual[VERSIONS]).hexdigest()
        print(f"{root.name}: {len(actual)} files match; raw {VERSIONS} SHA-256 {digest}")


if __name__ == "__main__":
    main()
