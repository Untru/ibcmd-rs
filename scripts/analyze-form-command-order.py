#!/usr/bin/env python3
"""Count Form Command child ordering without retaining application XML."""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
import xml.etree.ElementTree as ET


FUNCTIONAL_OPTIONS = "FunctionalOptions"
REPRESENTATION = "Representation"


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def classify_command(command: ET.Element) -> str | None:
    children = [local_name(child.tag) for child in command]
    functional = [
        index for index, name in enumerate(children) if name == FUNCTIONAL_OPTIONS
    ]
    representation = [
        index for index, name in enumerate(children) if name == REPRESENTATION
    ]
    if not functional or not representation:
        return None
    if len(functional) != 1 or len(representation) != 1:
        return "other"
    if functional[0] < representation[0]:
        return "functional_options_before_representation"
    return "representation_before_functional_options"


def analyze_form(path: Path) -> tuple[Counter[str], dict[tuple[str, str, int], str]]:
    root = ET.parse(path).getroot()
    counts: Counter[str] = Counter()
    identities: dict[tuple[str, str, int], str] = {}
    occurrences: defaultdict[tuple[str, str], int] = defaultdict(int)
    for element in root.iter():
        if local_name(element.tag) != "Command":
            continue
        counts["commands_total"] += 1
        classification = classify_command(element)
        if classification is None:
            continue
        counts["with_both"] += 1
        counts[classification] += 1
        base = (element.get("id", ""), element.get("name", ""))
        occurrence = occurrences[base]
        occurrences[base] += 1
        identities[(base[0], base[1], occurrence)] = classification
    return counts, identities


def resolve_inputs(args: argparse.Namespace) -> tuple[Path, Path, list[str]]:
    diff = json.loads(args.canonical_diff.read_text(encoding="utf-8-sig"))
    native_root = args.native_root or Path(diff["left_root"])
    candidate_root = args.candidate_root or Path(diff["right_root"])
    paths = sorted(
        {
            item["path"]
            for item in diff["differences"]
            if item["path"].replace("\\", "/").endswith("/Ext/Form.xml")
        }
    )
    return native_root, candidate_root, paths


def empty_summary() -> Counter[str]:
    return Counter(
        {
            "commands_total": 0,
            "with_both": 0,
            "functional_options_before_representation": 0,
            "representation_before_functional_options": 0,
            "other": 0,
        }
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Aggregate FunctionalOptions/Representation order in Form Commands "
            "from a canonical source-diff corpus."
        )
    )
    parser.add_argument("--canonical-diff", type=Path, required=True)
    parser.add_argument("--native-root", type=Path)
    parser.add_argument("--candidate-root", type=Path)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--candidate-commit", required=True)
    parser.add_argument("--analysis-parent-commit", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    native_root, candidate_root, relative_paths = resolve_inputs(args)
    summaries = {"native": empty_summary(), "candidate": empty_summary()}
    matched_pairs: Counter[str] = Counter()
    files = Counter()

    for relative in relative_paths:
        native_path = native_root / relative
        candidate_path = candidate_root / relative
        if not native_path.is_file() or not candidate_path.is_file():
            files["skipped_missing_side"] += 1
            continue
        files["analyzed"] += 1
        native_counts, native_commands = analyze_form(native_path)
        candidate_counts, candidate_commands = analyze_form(candidate_path)
        summaries["native"].update(native_counts)
        summaries["candidate"].update(candidate_counts)
        shared = native_commands.keys() & candidate_commands.keys()
        matched_pairs["shared_with_both"] += len(shared)
        matched_pairs["native_only_with_both"] += len(
            native_commands.keys() - candidate_commands.keys()
        )
        matched_pairs["candidate_only_with_both"] += len(
            candidate_commands.keys() - native_commands.keys()
        )
        for identity in shared:
            pair = (native_commands[identity], candidate_commands[identity])
            if pair == (
                "functional_options_before_representation",
                "representation_before_functional_options",
            ):
                matched_pairs["expected_native_vs_candidate_reverse"] += 1
            else:
                matched_pairs["contradictions"] += 1

    native = summaries["native"]
    candidate = summaries["candidate"]
    claim_contradictions = (
        native["representation_before_functional_options"]
        + native["other"]
        + candidate["functional_options_before_representation"]
        + candidate["other"]
        + matched_pairs["native_only_with_both"]
        + matched_pairs["candidate_only_with_both"]
        + matched_pairs["contradictions"]
    )
    result = {
        "schema_version": 1,
        "analysis": "form_command_functional_options_representation_order",
        "source": {
            "run_id": args.run_id,
            "canonical_diff": str(args.canonical_diff.resolve()),
            "native_root": str(native_root.resolve()),
            "candidate_root": str(candidate_root.resolve()),
            "candidate_commit": args.candidate_commit,
            "analysis_parent_commit": args.analysis_parent_commit,
        },
        "files": {
            "listed_form_xml": len(relative_paths),
            **dict(sorted(files.items())),
        },
        "native": dict(sorted(native.items())),
        "candidate": dict(sorted(candidate.items())),
        "matched_pairs": dict(sorted(matched_pairs.items())),
        "claim_contradictions": claim_contradictions,
    }
    encoded = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8", newline="\n")
    else:
        print(encoded, end="")
    return 0 if claim_contradictions == 0 else 2


if __name__ == "__main__":
    raise SystemExit(main())
