"""Read-only raw object-header measurements, with a public FileDb map.

The caller must supply a closed, immutable infobase snapshot. Offset 12 is
recorded verbatim; no version/count semantics are assumed for that word.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess

CONFIG_TABLES = ("CONFIG", "CONFIGSAVE", "PARAMS", "FILES", "CONFIGCAS", "CONFIGCASSAVE")


def read_page(stream, page_size, page_count, page):
    if page <= 0 or page >= page_count:
        raise ValueError("Page pointer outside file")
    stream.seek(page * page_size)
    value = stream.read(page_size)
    if len(value) != page_size:
        raise ValueError("Truncated page")
    return value


def read_object(stream, page_size, page_count, header_page):
    raw = read_page(stream, page_size, page_count, header_page)
    signature, fat, word4, word8, word12, length = struct.unpack_from("<HHIIIQ", raw)
    if signature != 0xFD1C or fat not in (0, 1):
        raise ValueError("Unsupported object signature/FAT level")
    need = (length + page_size - 1) // page_size
    if need > page_count:
        raise ValueError("Object length exceeds file capacity")
    direct = struct.unpack_from(f"<{(page_size - 24) // 4}I", raw, 24)
    fat_pages = []
    data_pages = []
    if fat == 0:
        if need > len(direct):
            raise ValueError("FAT0 capacity exceeded")
        data_pages = list(direct[:need])
    else:
        per = page_size // 4
        count = (need + per - 1) // per
        if count > len(direct):
            raise ValueError("FAT1 capacity exceeded")
        fat_pages = list(direct[:count])
        if len(set(fat_pages)) != len(fat_pages) or header_page in fat_pages:
            raise ValueError("Aliased FAT pages")
        for page in fat_pages:
            table = read_page(stream, page_size, page_count, page)
            entries = struct.unpack(f"<{per}I", table)
            data_pages.extend(entries[:min(per, need - len(data_pages))])
    if len(data_pages) != need or len(set(data_pages)) != len(data_pages):
        raise ValueError("Missing or repeated data pages")
    for page in data_pages:
        if page <= 0 or page >= page_count or page == header_page or page in fat_pages:
            raise ValueError("Invalid or aliased data page")
    digest = hashlib.sha256()
    for page in data_pages:
        digest.update(struct.pack("<I", page))
    return {
        "header_page": header_page, "fat_level": fat,
        "u32_4": word4, "u32_8": word8, "u32_12": word12,
        "length": length, "used_data_pages": len(data_pages),
        "required_fat_pages": len(fat_pages), "fat_page_numbers": fat_pages,
        "nonzero_direct_slots": sum(page != 0 for page in direct),
        "data_page_vector_sha256": digest.hexdigest(),
    }


def inspect(database, probe):
    source = database / "1Cv8.1CD" if database.is_dir() else database
    before = source.stat()
    process = subprocess.run([str(probe.resolve()), str(source.resolve())],
                             check=True, capture_output=True)
    tables, rows, metadata = {}, {}, None
    for line in process.stdout.decode("utf-8").splitlines():
        fields = line.split("\t")
        if fields[0] == "FILE" and len(fields) == 4 and metadata is None:
            metadata = {"locale": fields[1], "page_size": int(fields[2]), "page_count": int(fields[3])}
        elif fields[0] == "TABLE" and len(fields) == 5 and fields[1] not in tables:
            tables[fields[1]] = list(map(int, fields[2:]))
        elif fields[0] == "ROWS" and len(fields) == 5 and fields[1] not in rows:
            rows[fields[1]] = dict(zip(("parts", "stored_bytes", "largest_stored_part"),
                                       map(int, fields[2:])))
        else:
            raise ValueError("Unexpected/duplicate public probe record")
    if metadata is None:
        raise ValueError("Missing file metadata")
    ps, pages = metadata["page_size"], metadata["page_count"]
    if ps not in (4096, 8192, 16384, 32768, 65536) or before.st_size != ps * pages:
        raise ValueError("Unsupported page size or physical file length")
    objects = {}
    with source.open("rb") as stream:
        header = stream.read(24)
        if header[:12] != b"1CDBMSV8\x08\x03\x08\x00":
            raise ValueError("Unexpected file format")
        if struct.unpack_from("<I", header, 12)[0] != pages or struct.unpack_from("<I", header, 20)[0] != ps:
            raise ValueError("Raw file header differs from public reader")
        objects["ROOT"] = read_object(stream, ps, pages, 2)
        for name in CONFIG_TABLES:
            if name not in tables or name not in rows:
                raise ValueError("Missing configuration table")
            for role, page in zip(("records", "blob", "index"), tables[name]):
                if not page:
                    raise ValueError("Missing configuration object")
                objects[f"{name}/{role}"] = read_object(stream, ps, pages, page)
        stream.seek(0)
        file_hash = hashlib.file_digest(stream, "sha256").hexdigest()
    after = source.stat()
    if (before.st_size, before.st_mtime_ns, before.st_ino) != (after.st_size, after.st_mtime_ns, after.st_ino):
        raise ValueError("Input snapshot changed during inspection")
    return {
        "schema_version": 1, "file": metadata, "file_bytes": before.st_size,
        "file_sha256": file_hash, "objects": objects, "rows": rows,
        "probe_sha256": hashlib.sha256(probe.read_bytes()).hexdigest(),
        "probe_stdout_sha256": hashlib.sha256(process.stdout).hexdigest(),
        "inspector_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", required=True, type=Path)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = inspect(args.database, args.probe)
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        stream.write(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"file": result["file"], "objects": len(result["objects"]), "rows": result["rows"]}))


if __name__ == "__main__":
    main()
