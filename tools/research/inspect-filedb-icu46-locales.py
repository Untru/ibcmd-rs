"""Measure candidate-profile substitution without admitting additional locales.

Requires the same pinned Windows x64 runtime and public Rust probe as
compare-filedb-icu46.py. No native infobase or tool1cd executable is used.
"""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess


SOURCE = Path(__file__).with_name("compare-filedb-icu46.py")
SPEC = importlib.util.spec_from_file_location("icu_compare", SOURCE)
ICU = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ICU)
LOCALES = ("en_US", "de_DE", "tr_TR", "sv_SE")
PROFILES = ("en_US_POSIX", "ru_RU")


def full_key(runtime, collator, name, part):
    value = runtime.sort_key(collator, name) + ICU.part_bytes(part)
    if len(value) > 392:
        raise RuntimeError("Composite key exceeds CONFIG key length")
    return value + bytes(392 - len(value))


def measure(runtime, probe):
    baseline = ICU.run(runtime, probe)
    if baseline["mismatch_count"]:
        raise RuntimeError("The supported-profile baseline has mismatches")
    corpus = ICU.names()
    rows = [(locale, part, name) for locale in PROFILES + LOCALES
            for name in corpus for part in ICU.PARTS]
    rows += [("zz_ZZ", part, name) for name in ("I", "i", "root") for part in ICU.PARTS]
    wire = "".join(f"{locale}\t{part}\t{name}\n" for locale, part, name in rows).encode("utf-8")
    process = subprocess.run([str(probe.resolve())], input=wire, capture_output=True, check=True)
    replies = process.stdout.decode("ascii").splitlines()
    if len(replies) != len(rows):
        raise RuntimeError("Probe response count differs from request count")
    candidates = {}
    refused = 0
    for row, reply in zip(rows, replies):
        if row[0] not in PROFILES:
            if reply != "REFUSED":
                raise RuntimeError("An unmeasured locale was not refused")
            refused += 1
        else:
            if not reply.startswith("OK\t"):
                raise RuntimeError("Supported-profile candidate was refused")
            value = bytes.fromhex(reply[3:])
            if len(value) != 392:
                raise RuntimeError("Candidate key length differs from 392")
            candidates[row] = value
    # The baseline is a separate process invocation. Validate this invocation
    # too, so a changed/corrupted response cannot become substitution evidence.
    for profile in PROFILES:
        collator, _ = runtime.collator(profile)
        try:
            for name in corpus:
                for part in ICU.PARTS:
                    if candidates[(profile, part, name)] != full_key(runtime, collator, name, part):
                        raise RuntimeError("Candidate differs from pinned ICU")
        finally:
            runtime.close(collator)
    measurements = {}
    for locale in LOCALES:
        collator, metadata = runtime.collator(locale)
        try:
            if metadata["valid_locale"].split("_")[0] != locale.split("_")[0]:
                raise RuntimeError(f"Requested locale resolved to another language: {locale}")
            profiles = {profile: {"equal": 0, "different": 0, "first_difference": None}
                        for profile in PROFILES}
            oracle_hash = hashlib.sha256()
            for name in corpus:
                for part in ICU.PARTS:
                    expected = full_key(runtime, collator, name, part)
                    oracle_hash.update(expected)
                    for profile in PROFILES:
                        actual = candidates[(profile, part, name)]
                        result = profiles[profile]
                        if expected == actual:
                            result["equal"] += 1
                        else:
                            result["different"] += 1
                            if result["first_difference"] is None:
                                result["first_difference"] = {
                                    "name": name, "part": part,
                                    "first_byte_offset": next(i for i, pair in enumerate(zip(expected, actual))
                                                              if pair[0] != pair[1]),
                                    "icu_prefix_16": expected[:16].hex(),
                                    "candidate_prefix_16": actual[:16].hex(),
                                }
            measurements[locale] = {
                "metadata": metadata, "candidate_profiles": profiles,
                "oracle_full_keys_sha256": oracle_hash.hexdigest(),
                "I_i_sort_keys": {name: runtime.sort_key(collator, name).hex() for name in ("I", "i")},
            }
        finally:
            runtime.close(collator)
    collator, fallback = runtime.collator("zz_ZZ")
    runtime.close(collator)
    if fallback["valid_locale"].split("_")[0] == "zz":
        raise RuntimeError("The invalid-locale fallback control unexpectedly resolved")
    return {
        "schema_version": 1, "icu_version": runtime.version,
        "dll_sha256": ICU.DLL_SHA256,
        "helper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "baseline_helper_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        "probe_sha256": hashlib.sha256(probe.read_bytes()).hexdigest(),
        "baseline": {key: baseline[key] for key in
                     ("full_key_comparisons", "refusal_checks", "mismatch_count")},
        "distinct_names": len(corpus), "parts": list(ICU.PARTS), "key_bytes": 392,
        "per_locale_template_comparisons": len(corpus) * len(ICU.PARTS),
        "template_comparisons": len(corpus) * len(ICU.PARTS) * len(PROFILES) * len(LOCALES),
        "unsupported_refusal_checks": refused,
        "second_invocation_verified_candidate_keys": len(candidates),
        "input_sha256": hashlib.sha256(wire).hexdigest(),
        "probe_output_sha256": hashlib.sha256(process.stdout).hexdigest(),
        "locales": measurements, "fallback_control": {"requested": "zz_ZZ", **fallback},
        "fallback_counted_as_locale_evidence": False,
        "native_platform_run": False, "tool1cd_executable_run": False,
        "additional_locales_admitted": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--icu-root", required=True, type=Path)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = measure(ICU.ICU46(args.icu_root), args.probe)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({locale: report["locales"][locale]["candidate_profiles"] for locale in LOCALES}))


if __name__ == "__main__":
    main()
