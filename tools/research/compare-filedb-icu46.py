"""Compare the public config_key API with pinned ICU 4.6 Windows x64 DLLs.

No runtime is downloaded or bundled. Use the official Win64-msvc10 archive;
see docs/research/tool1cd-comparison.md for provenance and reproduction.
"""

import argparse
import ctypes as c
import hashlib
import itertools
import json
import os
from pathlib import Path
import subprocess
import sys


DLL_SHA256 = {
    "icudt46.dll": "2f7e6b454038073255b2c22b5b513e23bfa717ff60477b851474708950be8dd0",
    "icuuc46.dll": "34b919a3ddbac34200f9202b4b42398b25da248a2e98db7f63ba7acb09b53144",
    "icuin46.dll": "c5814166c52f474171362780f281b080a8e91b543dfe20f20aed26500f0ed001",
}
ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-./_"
PARTS = (0, 1, 4294967295)


def bind(dll, name, args, result):
    function = getattr(dll, name + "_46")
    function.argtypes = args
    function.restype = result
    return function


class ICU46:
    def __init__(self, root):
        if sys.platform != "win32" or c.sizeof(c.c_void_p) != 8:
            raise RuntimeError("This pinned runtime requires Windows x64 Python")
        self.directory = root.resolve() / "bin64"
        for name, expected in DLL_SHA256.items():
            actual = hashlib.sha256((self.directory / name).read_bytes()).hexdigest()
            if actual != expected:
                raise RuntimeError(f"Unrecognized ICU DLL: {name}: {actual}")
        if os.environ.get("ICU_DATA"):
            raise RuntimeError("Unset ICU_DATA to avoid overriding the pinned data")
        self.search = os.add_dll_directory(str(self.directory))
        self.data = c.CDLL(str(self.directory / "icudt46.dll"))
        self.common = c.CDLL(str(self.directory / "icuuc46.dll"))
        self.intl = c.CDLL(str(self.directory / "icuin46.dll"))
        version = (c.c_uint8 * 4)()
        bind(self.common, "u_getVersion", [c.c_void_p], None)(version)
        self.version = list(version)
        if self.version != [4, 6, 0, 0]:
            raise RuntimeError(f"Unexpected ICU version: {self.version}")
        # Use the package directory, not a working-directory data override.
        bind(self.common, "u_setDataDirectory", [c.c_char_p], None)(
            os.fsencode(self.directory)
        )
        self.open = bind(self.intl, "ucol_open", [c.c_char_p, c.c_void_p], c.c_void_p)
        self.close = bind(self.intl, "ucol_close", [c.c_void_p], None)
        self.strength = bind(self.intl, "ucol_setStrength", [c.c_void_p, c.c_int32], None)
        self.key = bind(
            self.intl, "ucol_getSortKey",
            [c.c_void_p, c.c_void_p, c.c_int32, c.c_void_p, c.c_int32], c.c_int32,
        )
        self.attribute = bind(
            self.intl, "ucol_getAttribute", [c.c_void_p, c.c_int32, c.c_void_p], c.c_int32
        )
        self.locale = bind(
            self.intl, "ucol_getLocaleByType", [c.c_void_p, c.c_int32, c.c_void_p], c.c_char_p
        )

    def collator(self, locale):
        status = c.c_int32(0)
        coll = self.open(locale.encode("ascii"), c.byref(status))
        if not coll or status.value > 0:
            raise RuntimeError(f"ucol_open({locale}): {status.value}")
        self.strength(coll, 1)  # UCOL_SECONDARY, as in tool1cd's CI field.
        info = {"open_status": status.value}
        for label, function in (("collation_version", "ucol_getVersion"),
                                ("uca_version", "ucol_getUCAVersion")):
            version = (c.c_uint8 * 4)()
            bind(self.intl, function, [c.c_void_p, c.c_void_p], None)(coll, version)
            info[label] = list(version)
        for label, kind in (("valid_locale", 1), ("actual_locale", 0)):
            status = c.c_int32(0)
            value = self.locale(coll, kind, c.byref(status))
            if status.value > 0 or value is None:
                raise RuntimeError(f"{label}: {status.value}")
            info[label] = value.decode("ascii")
        info["attributes"] = {}
        for attribute in range(8):
            status = c.c_int32(0)
            value = self.attribute(coll, attribute, c.byref(status))
            if status.value > 0:
                raise RuntimeError(f"attribute {attribute}: {status.value}")
            info["attributes"][str(attribute)] = value
        return coll, info

    def sort_key(self, coll, name):
        # tool1cd trims spaces before ICU. The admitted domain has none.
        encoded = name.rstrip(" ").encode("utf-16-le")
        source = c.create_string_buffer(encoded)
        units = len(encoded) // 2
        required = self.key(coll, source, units, None, 0)
        if not 1 <= required <= 386:  # NVC(128): length * 3 + 2.
            raise RuntimeError(f"Unexpected ICU sort-key length: {required}")
        output = (c.c_uint8 * required)()
        if self.key(coll, source, units, output, required) != required or output[-1] != 0:
            raise RuntimeError("ICU sort key changed size or lacks terminator")
        return bytes(output)


def part_bytes(part):
    # Positive N(10): sign nibble 1, ten decimal digits, final padding nibble 0.
    return bytes.fromhex("1" + f"{part:010d}" + "0")


def names():
    corpus = set(ALPHABET)
    corpus.update("".join(pair) for pair in itertools.product(ALPHABET, repeat=2))
    corpus.update(character * length for character in ALPHABET for length in (42, 43))
    corpus.update((ALPHABET * 2)[:length] for length in range(1, 44))
    corpus.update(("-./_" * 11)[:length] for length in range(1, 44))
    corpus.update(("DBNames", "dbnames", "root", "ROOT", "version", "versions",
                   "configinfo", "ConfigInfo", "00000000-0000-0000-0000-000000000000",
                   "a/b.c_d-0", "A/B.C_D-0"))
    for i in range(512):
        digest = hashlib.sha512(f"issue525:{i}".encode("ascii")).digest()
        corpus.add("".join(ALPHABET[byte % len(ALPHABET)] for byte in digest[:1 + i % 43]))
    return sorted(corpus)


def run(runtime, probe):
    assert [part_bytes(part).hex() for part in PARTS] == [
        "100000000000", "100000000010", "142949672950"
    ]
    corpus = names()
    requests, expected, metadata = [], [], {}
    for locale in ("en_US_POSIX", "ru_RU"):
        coll, metadata[locale] = runtime.collator(locale)
        try:
            for name in corpus:
                sort_key = runtime.sort_key(coll, name)
                for part in PARTS:
                    requests.append(f"{locale}\t{part}\t{name}\n")
                    key = sort_key + part_bytes(part)
                    expected.append(key + bytes(392 - len(key)))
        finally:
            runtime.close(coll)
    comparisons = len(requests)
    for locale in ("en_US_POSIX", "ru_RU"):
        for name in ("", "x" * 44, " ", "root ", "а", "é", "\0"):
            requests.append(f"{locale}\t0\t{name}\n")
            expected.append(None)
    for part in PARTS:
        requests.append(f"de_DE\t{part}\troot\n")
        expected.append(None)
    coll, metadata["de_DE"] = runtime.collator("de_DE")
    try:
        metadata["de_DE"]["ICU_only_sort_keys"] = {
            name: runtime.sort_key(coll, name).hex() for name in ("root", "ä", "Z")
        }
    finally:
        runtime.close(coll)
    wire = "".join(requests).encode("utf-8")
    result = subprocess.run([str(probe.resolve())], input=wire, capture_output=True, check=True)
    lines = result.stdout.decode("ascii").splitlines()
    if len(lines) != len(expected):
        raise RuntimeError(f"Probe returned {len(lines)} rows for {len(expected)} inputs")
    mismatches = []
    for request, wanted, actual in zip(requests, expected, lines):
        wanted_line = "REFUSED" if wanted is None else "OK\t" + wanted.hex()
        if actual != wanted_line:
            mismatches.append({"input": request.rstrip("\n"),
                               "expected": wanted_line, "actual": actual})
    return {
        "icu_version": runtime.version, "dll_sha256": DLL_SHA256,
        "runtime_archive": "icu4c-4_6-Win64-msvc10.zip",
        "runtime_archive_sha256": "c64bd3b9fade2c7cefbb1c8c16f765febda8b0f4de771d5f50d88a150d1cf41b",
        "key_bytes": 392, "parts": list(PARTS),
        "probe_sha256": hashlib.sha256(probe.read_bytes()).hexdigest(),
        "locales": metadata, "distinct_names": len(corpus),
        "full_key_comparisons": comparisons, "refusal_checks": len(expected) - comparisons,
        "input_sha256": hashlib.sha256(wire).hexdigest(),
        "probe_output_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "mismatch_count": len(mismatches), "first_mismatches": mismatches[:10],
        "native_platform_run": False, "tool1cd_executable_run": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--icu-root", required=True, type=Path)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = run(ICU46(args.icu_root), args.probe)
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")
    print(json.dumps({key: report[key] for key in (
        "distinct_names", "full_key_comparisons", "refusal_checks", "mismatch_count"
    )}))
    return int(report["mismatch_count"] != 0)


if __name__ == "__main__":
    sys.exit(main())
