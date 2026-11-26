"""Compare a full SQL export with native XML, then import both into CF-seeded twins."""
import argparse
import base64
import hashlib
import json
import os
import subprocess
import uuid
from pathlib import Path
from xml.etree import ElementTree as ET


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inventory(root):
    return {p.relative_to(root).as_posix(): digest(p.read_bytes())
            for p in root.rglob("*") if p.is_file()}


def compare(left, right):
    return {
        "equal": sum(left[k] == right[k] for k in left.keys() & right.keys()),
        "changed": sorted(k for k in left.keys() & right.keys() if left[k] != right[k]),
        "missing": sorted(left.keys() - right.keys()),
        "extra": sorted(right.keys() - left.keys()),
    }


def xml_fields(path):
    """Index XML names by expanded name and sibling occurrence within that name."""
    fields = {}

    def visit(node, key):
        fields[key] = node.text if node.text and node.text.strip() else ""
        for name, value in node.attrib.items():
            fields[f"{key}/@{name}"] = value
        counts = {}
        for child in node:
            counts[child.tag] = counts.get(child.tag, 0) + 1
            visit(child, f"{key}/{child.tag}[{counts[child.tag]}]")

    if path.is_file():
        root = ET.parse(path).getroot()
        visit(root, root.tag)
    return fields


def field_census(left, right, names):
    counts = dict(equal=0, changed=0, missing=0, extra=0)
    differences = {}
    for name in sorted(names):
        if not name.endswith(".xml"):
            continue
        before, after = xml_fields(left / name), xml_fields(right / name)
        delta = compare(before, after)
        counts["equal"] += delta["equal"]
        for status in ("changed", "missing", "extra"):
            counts[status] += len(delta[status])
            for key in delta[status]:
                differences.setdefault(name, {})[key] = [before.get(key), after.get(key)]
    return counts, differences


def sql_text(server, database, query):
    return subprocess.check_output([
        "sqlcmd", "-S", server, "-d", database, "-E", "-C", "-b", "-f", "65001", "-w", "65535",
        "-W", "-h", "-1", "-s", "|", "-Q", query,
    ]).decode("utf-8-sig")


def save_raw_rows(server, database, raw, original):
    # Split the hex data below sqlcmd's line width even for large template bodies.
    raw.mkdir()
    query = """
    SET NOCOUNT ON;
    WITH chunks AS (
        SELECT FileName, PartNo, 1 AS pos, DATALENGTH(BinaryData) AS size FROM dbo.Config
        UNION ALL
        SELECT FileName, PartNo, pos + 1024, size FROM chunks WHERE pos + 1024 <= size
    )
    SELECT RTRIM(c.FileName), c.PartNo, c.pos,
           CONVERT(varchar(2048), SUBSTRING(CONVERT(varbinary(max), r.BinaryData), c.pos, 1024), 2)
    FROM chunks c JOIN dbo.Config r ON r.FileName=c.FileName AND r.PartNo=c.PartNo
    ORDER BY c.FileName, c.PartNo, c.pos OPTION (MAXRECURSION 0);
    """
    bodies = {}
    for line in sql_text(server, database, query).splitlines():
        if line.strip():
            name, part, pos, chunk = line.strip().split("|")
            key = f"{name}__part{part}"
            body = bodies.setdefault(key, bytearray())
            if len(body) + 1 != int(pos):
                raise ValueError(f"discontinuous SQL row {key}")
            body.extend(bytes.fromhex(chunk))
    if original.keys() != bodies.keys():
        raise ValueError("incomplete Config snapshot")
    for key, body in bodies.items():
        _, size, sha = original[key]
        if Path(key).name != key or len(body) != size or digest(body).upper() != sha:
            raise ValueError(f"invalid Config snapshot row {key}")
        (raw / (key + ".bin")).write_bytes(body)


def config_rows(server, database):
    query = """
    SET NOCOUNT ON;
    SELECT RTRIM(FileName), PartNo, DataSize, DATALENGTH(BinaryData),
           CONVERT(varchar(64), HASHBYTES('SHA2_256', CONVERT(varbinary(max), BinaryData)), 2)
    FROM dbo.Config ORDER BY FileName, PartNo;
    """
    output = sql_text(server, database, query)
    rows = {}
    for line in output.splitlines():
        if line.strip():
            name, part, size, stored, sha = line.strip().split("|")
            rows[f"{name}__part{part}"] = [int(size), int(stored), sha]
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--cf-base64", type=Path)
    inputs.add_argument("--cf", type=Path)
    parser.add_argument("--ibcmd", required=True, type=Path)
    parser.add_argument("--exporter", required=True, type=Path)
    parser.add_argument("--server", default="localhost")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    root = args.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    fixture = root / "input.cf"
    fixture.write_bytes(args.cf.read_bytes() if args.cf else
                        base64.b64decode(args.cf_base64.read_bytes()))
    prefix = "i472_" + uuid.uuid4().hex[:12]
    source = prefix + "_source"
    native, ours = root / "native", root / "ours"

    def platform(db, *command, options=()):
        subprocess.run([args.ibcmd, "infobase", *command, "--dbms=MSSQLServer",
                        "--db-server=" + args.server, "--db-name=" + db,
                        "--data=" + str(root / (db + "-data")), *options], check=True)

    def seed(db):
        platform(db, "create", options=["--create-database", "--load=" + str(fixture)])
        platform(db, "config", "apply", options=["--force", "--dynamic=disable"])

    seed(source)
    platform(source, "config", "export", options=["--threads=1", native])
    xml_version = ET.parse(native / "Configuration.xml").getroot().attrib["version"]
    expected = inventory(native)
    original = config_rows(args.server, source)
    if not expected or not original:
        raise ValueError("empty native reference")
    raw = root / "Config"
    save_raw_rows(args.server, source, raw, original)
    env = {**os.environ, "IBCMD_RS_MODEL_EXPORT_SHADOW": "1", "IBCMD_RS_MODEL_EXPORT": "1",
           "IBCMD_RS_LEGACY_EXPORT": "0"}
    with (root / "export.json").open("wb") as output, (root / "export.log").open("wb") as log:
        export = subprocess.run([args.exporter, "mssql-dump-config", "--rows-dir", raw, "-o", ours,
                        "--no-binary-rows",
                        "--extract-metadata-xml", "--extract-module-text",
                        "--source-version", xml_version], stdout=output, stderr=log, env=env)
    # The diagnostic manifest is outside the platform's XML source tree.
    if (ours / "manifest.json").is_file():
        (ours / "manifest.json").rename(root / "diagnostic-manifest.json")
    audit_path = root / "descriptor-audit.json"
    subprocess.run([args.exporter, "audit-metadata-export", native, raw, "-o", audit_path],
                   check=True)
    audit = json.loads(audit_path.read_text(encoding="utf-8"))
    result = {
        "platform": subprocess.check_output([args.ibcmd, "--version"]).decode().strip(),
        "xml_version": xml_version,
        "fixture_sha256": digest(fixture.read_bytes()),
        "exporter_sha256": digest(args.exporter.read_bytes()),
        "descriptor_audit": {kind: {k: counts[k] for k in
                             ("total", "identical", "different", "failed", "no_row", "lossless", "lossy")}
                             for kind, counts in audit["kinds"].items()},
        "rows": len(original), "export_exit": export.returncode,
        "source_xml": compare(expected, inventory(ours)),
    }
    if export.returncode == 0:
        table, = json.loads((root / "export.json").read_text(encoding="utf-8-sig"))["tables"]
        result["source_asset_rows"] = table["source_asset_rows"]
        result["descriptor_counters"] = {k: v for k, v in table["timings"].items()
                                         if k.startswith("model_") and k.endswith("_rows")}
    else:
        result["export_errors"] = [line for line in (root / "export.log").read_text(
            encoding="utf-8").splitlines() if line.startswith("Error:")]
    counts, differences = field_census(native, ours, expected.keys() | inventory(ours).keys())
    result["xml_field_counts"] = counts
    (root / "field-differences.json").write_text(
        json.dumps(differences, ensure_ascii=False, indent=2), encoding="utf-8")
    result["fallback_messages"] = [
        line for line in (root / "export.log").read_text(encoding="utf-8").splitlines()
        if line.startswith("model export: ") and line.endswith("the legacy converter writes it")
    ]
    result["platform_steps"] = {}
    snapshots = {}
    trees = {}
    for side, xml in [("native", native), ("ours", ours)]:
        if side == "ours" and export.returncode:
            continue
        db = prefix + "_" + side
        seed(db)
        tree = root / (side + "-reexport")
        steps = result["platform_steps"][side] = {}
        for operation, options in [("import", ["--threads=1", xml]),
                                   ("apply", ["--force", "--dynamic=disable"]),
                                   ("export", ["--threads=1", tree])]:
            try:
                platform(db, "config", operation, options=options)
            except subprocess.CalledProcessError as error:
                steps[operation] = error.returncode
                break
            steps[operation] = 0
        if steps.get("export") != 0:
            continue
        snapshots[side] = config_rows(args.server, db)
        trees[side] = inventory(tree)
        result[side + "_vs_original_config"] = compare(original, snapshots[side])
        result[side + "_vs_original_xml"] = compare(expected, trees[side])
    if len(snapshots) == 2:
        result["imported_config"] = compare(snapshots["native"], snapshots["ours"])
        result["imported_xml"] = compare(trees["native"], trees["ours"])
    result["template_bodies"] = {
        p.stem: ET.parse(p).getroot()[0].attrib["uuid"] + ".0__part0"
        for p in sorted((native / "CommonTemplates").glob("*.xml"))
    }
    for side, rows in [("source", original), *snapshots.items()]:
        (root / (side + "-rows.json")).write_text(json.dumps(rows, indent=2), encoding="utf-8")
    (root / "census.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n",
                                      encoding="utf-8")
    if len(snapshots) != 2:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
