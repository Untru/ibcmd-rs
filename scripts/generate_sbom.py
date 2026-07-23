#!/usr/bin/env python3
"""Generate a deterministic CycloneDX 1.5 SBOM from Cargo's locked graph."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import tomllib
from collections import deque


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest-path", default="Cargo.toml")
    parser.add_argument("--output", required=True)
    parser.add_argument("--target", required=True)
    return parser.parse_args()


def normal_dependency(dep: dict) -> bool:
    kinds = dep.get("dep_kinds") or []
    return not kinds or any(item.get("kind") in (None, "normal") for item in kinds)


def lock_checksums(root: pathlib.Path) -> dict[tuple[str, str, str | None], str]:
    with (root / "Cargo.lock").open("rb") as stream:
        document = tomllib.load(stream)
    result = {}
    for package in document.get("package", []):
        checksum = package.get("checksum")
        if checksum:
            result[(package["name"], package["version"], package.get("source"))] = checksum
    return result


def component(package: dict, checksum: str | None, root_id: str) -> dict:
    name = package["name"]
    version = package["version"]
    item = {
        "type": "application" if package["id"] == root_id else "library",
        "bom-ref": f"pkg:cargo/{name}@{version}",
        "name": name,
        "version": version,
        "purl": f"pkg:cargo/{name}@{version}",
    }
    if package.get("license"):
        item["licenses"] = [{"expression": package["license"]}]
    if checksum:
        item["hashes"] = [{"alg": "SHA-256", "content": checksum}]
    return item


def main() -> None:
    args = arguments()
    manifest = pathlib.Path(args.manifest_path).resolve()
    root = manifest.parent
    command = [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
        "--offline",
        "--filter-platform",
        args.target,
        "--no-default-features",
        "--manifest-path",
        str(manifest),
    ]
    metadata = json.loads(subprocess.check_output(command, cwd=root, text=True))
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root_package = next(
        package
        for package in metadata["packages"]
        if package["name"] == "ibcmd-rs" and pathlib.Path(package["manifest_path"]) == manifest
    )

    reachable = set()
    queue = deque([root_package["id"]])
    while queue:
        package_id = queue.popleft()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        for dep in nodes[package_id].get("deps", []):
            if normal_dependency(dep):
                queue.append(dep["pkg"])

    checksums = lock_checksums(root)
    components = []
    for package_id in sorted(reachable, key=lambda value: (packages[value]["name"], packages[value]["version"])):
        package = packages[package_id]
        checksum = checksums.get(
            (package["name"], package["version"], package.get("source"))
        )
        components.append(component(package, checksum, root_package["id"]))

    dependencies = []
    for package_id in sorted(reachable, key=lambda value: (packages[value]["name"], packages[value]["version"])):
        package = packages[package_id]
        refs = []
        for dep in nodes[package_id].get("deps", []):
            if dep["pkg"] in reachable and normal_dependency(dep):
                target = packages[dep["pkg"]]
                refs.append(f"pkg:cargo/{target['name']}@{target['version']}")
        dependencies.append(
            {
                "ref": f"pkg:cargo/{package['name']}@{package['version']}",
                "dependsOn": sorted(set(refs)),
            }
        )

    root_ref = f"pkg:cargo/{root_package['name']}@{root_package['version']}"
    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "tools": {
                "components": [
                    {
                        "type": "application",
                        "name": "ibcmd-rs deterministic Cargo SBOM generator",
                    }
                ]
            },
            "component": next(item for item in components if item["bom-ref"] == root_ref),
            "properties": [
                {"name": "ibcmd-rs:target", "value": args.target},
                {"name": "ibcmd-rs:default-features", "value": "disabled"},
            ],
        },
        "components": components,
        "dependencies": dependencies,
    }
    output = pathlib.Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(bom, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
