"""Compare a local #581 generated module with the checked-out StdPicture tables."""
from collections import Counter
import argparse
import json
import re
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UUID = r'[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}'
NAME = r'[A-Za-z][A-Za-z0-9]*'


def read_table(path, table, first, second, reverse=False):
    source = path.read_text(encoding="utf-8-sig")
    source = re.sub(r"//[^\n]*", "", source)
    bodies = re.findall(rf'\bconst\s+{table}\s*:[^=]+?=\s*&\[(.*?)\]\s*;', source, re.S)
    if len(bodies) != 1:
        raise ValueError(f"{path}: expected one {table}")
    row = re.compile(rf'\s*\(\s*{first}\s*,\s*{second}\s*,?\s*\)\s*,')
    body = bodies[0].rstrip()
    result = {}
    keys = set()
    pos = 0
    while pos < len(body):
        match = row.match(body, pos)
        if not match:
            raise ValueError(f"{table}: unsupported row near {body[pos:pos+100]!r}")
        key, name = reversed(match.groups()) if reverse else match.groups()
        key = key.lower()
        if key in keys or name in result:
            raise ValueError(f"{table}: duplicate key or name at {key}, {name}")
        result[name] = key
        keys.add(key)
        pos = match.end()
    if not result:
        raise ValueError(f"{table}: empty table")
    return result


def compare(repo, sample):
    left, right = set(repo.items()), set(sample.items())
    common = left & right
    by_key = {key: name for name, key in repo.items()}
    return {
        "repository": len(repo), "sample": len(sample), "exact": len(common),
        "repo_minus_sample": sorted(left - right),
        "sample_minus_repo": sorted(right - left),
        "same_name_different_key": sorted(
            (name, repo[name], key) for name, key in right if name in repo and repo[name] != key
        ),
        "same_key_different_name": sorted(
            (key, by_key[key], name) for name, key in right if key in by_key and by_key[key] != name
        ),
    }


def help_names(path):
    """Read PictureLib property titles from the embedded ZIP entries of shcntx_root.hbk."""
    data = path.read_bytes()
    names = set()
    # The digits in an article filename include its help ID. Read the actual title.
    article = rb"objects/catalog[0-9]+/PictureLib/properties/[^/\x00-\x20]+?\.html"
    local = Counter()
    directory = Counter()
    for match in re.finditer(article, data):
        central = match.start() - 46
        if data[central:central + 4] == b"PK\x01\x02":
            directory[match[0]] += 1
            continue
        header = match.start() - 30
        if data[header:header + 4] != b"PK\x03\x04":
            continue
        local[match[0]] += 1
        method = struct.unpack_from("<H", data, header + 8)[0]
        crc, size, raw_size, length, extra = struct.unpack_from("<IIIHH", data, header + 14)
        if method != 8 or length != len(match[0]):
            raise ValueError(f"unsupported help entry: {match[0]!r}")
        start = match.end() + extra
        html = zlib.decompress(data[start:start + size], -15)
        if len(html) != raw_size or zlib.crc32(html) != crc:
            raise ValueError(f"corrupt help entry: {match[0]!r}")
        title = re.search(rb"<h1[^>]*>PictureLib\.([^<]+)</h1>", html)
        if not title:
            raise ValueError(f"missing PictureLib title: {match[0]!r}")
        name = title[1].decode("utf-8")
        if name == "&lt;Icon name&gt;":
            continue
        if not re.fullmatch(NAME, name):
            raise ValueError(f"unrecognized PictureLib title: {name}")
        names.add(name)
    if local != directory:
        raise ValueError(f"{path}: incomplete PictureLib help entries")
    if not names:
        raise ValueError(f"{path}: no English PictureLib property titles")
    return names


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sample", type=Path)
    parser.add_argument("--help-book", type=Path, help="optional local English shcntx_root.hbk")
    args = parser.parse_args()
    uuid = f'"({UUID})"'
    name = rf'"StdPicture\.({NAME})"'
    code = r'(-[1-9][0-9]*)'
    uuids = ROOT / "crates/ibcmd-1c-vocab/src/pictures.rs"
    codes = ROOT / "crates/ibcmd-metadata-model/src/metadata_model/registers_parts.rs"
    sample_uuids = read_table(args.sample, "STANDARD_PICTURES", uuid, name)
    sample_codes = read_table(args.sample, "STANDARD_PICTURE_CODES", code, name)
    repo_uuids = read_table(uuids, "STANDARD_PICTURES", uuid, name)
    repo_codes = read_table(codes, "STANDARD_PICTURE_CODES", f'"({NAME})"', code, reverse=True)
    report = {"uuid": compare(repo_uuids, sample_uuids), "code": compare(repo_codes, sample_codes)}
    if args.help_book:
        titles = help_names(args.help_book)
        names = set(sample_uuids) | set(sample_codes)
        report["help"] = {
            "titles": len(titles), "common": len(names & titles),
            "sample_only": sorted(names - titles), "help_only": sorted(titles - names),
        }
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
