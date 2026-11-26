"""The forms of the objects a stage removes attributes from, without the fields bound to them: what the route through OUR import needs on top of
the stage the platform gets (the descriptors only). Reads <case dir>/stage (the edited descriptors) against the reference tree, and writes the
edited forms to <case dir>/stage_full/<relative path> and their list to <case dir>/forms.txt.

usage: python mix_forms.py <case dir>      (<case dir> is what edit_cases_s4.py wrote: stage/, before/, files.txt)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from edit_cases_s1 import REF  # noqa: E402
from form_bindings import drop_bindings  # noqa: E402


def attribute_names(text):
    """The names of the object-level attributes of a descriptor (three tabs deep)."""
    names = []
    for block in re.finditer(r"\n\t\t\t<Attribute uuid=.*?\n\t\t\t</Attribute>", text, re.S):
        names.append(re.search(r"<Name>(.*?)</Name>", block.group(0)).group(1))
    return names


def read(path):
    with open(path, "rb") as f:
        raw = f.read()
    return raw.decode("utf-8-sig"), raw.startswith(b"\xef\xbb\xbf")


def main():
    case_dir = sys.argv[1]
    stage = os.path.join(case_dir, "stage")
    written = {}

    def read_form(rel):
        return written[rel][0] if rel in written else read(os.path.join(REF, rel))[0]

    def write_form(rel, text):
        written[rel] = (text, read(os.path.join(REF, rel))[1])

    with open(os.path.join(case_dir, "files.txt"), encoding="utf-8") as f:
        files = [line.strip() for line in f if line.strip()]
    for rel in files:
        if not (rel.startswith("Catalogs/") or rel.startswith("Documents/")) or rel.count("/") != 1:
            continue
        before = set(attribute_names(read(os.path.join(REF, rel))[0]))
        after = set(attribute_names(read(os.path.join(stage, rel))[0]))
        removed = sorted(before - after)
        if removed:
            print(rel, "removes", ", ".join(removed))
            drop_bindings(read_form, write_form, rel[:-len(".xml")], removed)
    for rel, (text, bom) in written.items():
        path = os.path.join(case_dir, "stage_full", rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as f:
            f.write((b"\xef\xbb\xbf" if bom else b"") + text.encode("utf-8"))
    with open(os.path.join(case_dir, "forms.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(sorted(written)) + ("\n" if written else ""))
    print("%d form(s) edited" % len(written))


main()
