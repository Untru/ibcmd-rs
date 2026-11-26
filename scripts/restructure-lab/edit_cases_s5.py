"""S1-F in the combination matrix (issue #391, after S1-F #402): a stage that creates a catalog or a document and also changes other objects.

The platform stages a new object on a base that already had a native apply only (docs/apply/new-object.md 8): the base is the backup
bak\\ibcmd_rs_04_ddl_bsp8327_c2_native_after.bak, its native export is tree_c2. The new objects are the ones of the S1-F cases of the trace track
(newobj\\n1 a flat catalog, n4 a document, n5 a catalog and a document; copies of F:\\ibcmd\\lab\\05\\s1g\\n, made by scripts/apply-trace/lab/
s1g-caches/make_cases_n.py), their files and Configuration.xml go into the stage as they are, the changes of the other objects are made on
tree_c2 like the cases of edit_cases_s4.py.

Run the case with the base in the environment:
  $env:MIX_BASE_BAK = 'F:\\ibcmd\\lab\\04\\restructure\\bak\\ibcmd_rs_04_ddl_bsp8327_c2_native_after.bak'
  $env:MIX_BASE_TREE = 'F:\\ibcmd\\lab\\04\\restructure\\tree_c2'
  pwsh -NoProfile -File mix_case.ps1 -Case f1 [-Refused] [-Ours]

usage: python edit_cases_s5.py <out dir> <case>
  f1  a new catalog next to C and D on other objects: two widened strings, three switched indexes (a catalog, a hierarchical one, a document)
  f3  a new catalog and a new document next to A, C and D on other objects
  r7  REFUSED: a new document and an attribute removed from a catalog (the caches of a new object are chained on additions only)
  r8  REFUSED: a new catalog and new tabular sections (a section, an attribute of one) of existing objects
"""
import os
import shutil
import sys

os.environ.setdefault("DDL_NATIVE_TREE", r"F:\ibcmd\lab\04\restructure\tree_c2")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from edit_cases_s2 import BANKS, JOBS, KEYOPS, VOLUMES, PAYROLL, Tree, flip, remove_attribute, widen  # noqa: E402
from edit_cases_s3 import add_section_attribute, section_attribute, s  # noqa: E402
from edit_cases_s4 import CASH, INTAKE, PAYMENT, own, section_of  # noqa: E402
from form_bindings import drop_bindings  # noqa: E402

NEWOBJ = os.environ.get("MIX_NEWOBJ", r"F:\ibcmd\lab\04\restructure\newobj")
BOM = bytes([0xEF, 0xBB, 0xBF])


def add_use(text):
    """The attributes of a hand-made catalog file get the `<Use>ForItem</Use>` the exporter always writes (the platform's import takes a file
    without it, ours does not: `descriptor: no <Use>`). Only the route through our import gets these files (stage_full/)."""
    cr, lf, tab = chr(13), chr(10), chr(9)
    nl = cr + lf if cr + lf in text else lf
    lines = text.split(nl)
    out = []
    inside = False
    has_use = False
    for line in lines:
        if line.startswith(tab * 3 + "<Attribute "):
            inside, has_use = True, False
        elif line.startswith(tab * 3 + "</Attribute>"):
            inside = False
        elif inside and line.strip().startswith("<Use>"):
            has_use = True
        elif inside and line.strip().startswith("<Indexing>") and not has_use:
            out.append(line[: len(line) - len(line.lstrip())] + "<Use>ForItem</Use>")
            has_use = True
        out.append(line)
    return nl.join(out)


def new_objects(tree, case):
    """The files of a new-object case of S1-F, copied into the stage as they are."""
    source = os.path.join(NEWOBJ, case)
    with open(os.path.join(source, "files.txt"), encoding="utf-8") as f:
        files = [line.strip() for line in f if line.strip()]
    for rel in files:
        target = os.path.join(tree.stage, rel)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        shutil.copyfile(os.path.join(source, "stage", rel), target)
        print("new", rel)
        if rel.startswith("Catalogs/") and rel.count("/") == 1:
            with open(target, "rb") as f:
                raw = f.read()
            fixed = add_use(raw.decode("utf-8-sig"))
            full = os.path.join(tree.out, "stage_full", rel)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "wb") as f:
                f.write((BOM if raw.startswith(BOM) else b"") + fixed.encode("utf-8"))
    tree.extra = files


def run_f1(tree):
    new_objects(tree, "n1")
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)
    widen(tree, PAYMENT, "НазначениеПлатежа", 210, 300)
    flip(tree, BANKS, "Телефоны", "DontIndex", "IndexWithAdditionalOrder")
    flip(tree, VOLUMES, "МаксимальныйРазмер", "DontIndex", "Index")
    flip(tree, PAYMENT, "НомерВходящегоДокумента", "DontIndex", "Index")


def run_f3(tree):
    new_objects(tree, "n5")
    own(tree, JOBS, "КлючНовый", 64, "ForItem")
    own(tree, CASH)
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)
    flip(tree, VOLUMES, "МаксимальныйРазмер", "DontIndex", "Index")
    flip(tree, PAYROLL, "ПериодРегистрации", "IndexWithAdditionalOrder", "DontIndex")


def run_r7(tree):
    new_objects(tree, "n4")
    remove_attribute(tree, BANKS, "КоррСчет")
    widen(tree, KEYOPS, "ИмяХеш", 40, 100)


def run_r8(tree):
    new_objects(tree, "n1")
    section_of(tree, KEYOPS, "ДемоТЧ", [s("ДемоСтрока", 20)])
    add_section_attribute(tree, INTAKE, "Товары", section_attribute(*s("ДемоСтрока", 10, "Index")))
    widen(tree, BANKS, "Город", 50, 200)


CASES = {"f1": run_f1, "f3": run_f3, "r7": run_r7, "r8": run_r8}

if __name__ == "__main__":
    out, case = sys.argv[1], sys.argv[2]
    if case not in CASES:
        raise SystemExit("unknown case %s; known: %s" % (case, ", ".join(CASES)))
    tree = Tree(out)
    tree.extra = []
    CASES[case](tree)
    tree.save()
    # The platform's partial import wants the files an object refers to (its forms, templates) in the base dir when the stage carries
    # Configuration.xml: they go into the stage and stay out of files.txt.
    for rel in sorted(tree.files):
        folder = os.path.join(os.environ["DDL_NATIVE_TREE"], rel[: -len(".xml")])
        if not os.path.isdir(folder):
            continue
        for root, _dirs, names in os.walk(folder):
            for name in names:
                source = os.path.join(root, name)
                target = os.path.join(out, "stage", os.path.relpath(source, os.environ["DDL_NATIVE_TREE"]))
                if not os.path.exists(target) or case == "r7":
                    os.makedirs(os.path.dirname(target), exist_ok=True)
                    shutil.copyfile(source, target)
    # Configuration.xml makes the platform load copied forms too. Remove the
    # bindings to the deleted attribute on both import routes, before staging.
    if case == "r7":
        written = []

        def read_form(rel):
            with open(os.path.join(out, "stage", rel), "rb") as f:
                return f.read().decode("utf-8-sig")

        def write_form(rel, text):
            path = os.path.join(out, "stage", rel)
            with open(path, "rb") as f:
                bom = f.read().startswith(BOM)
            with open(path, "wb") as f:
                f.write((BOM if bom else b"") + text.encode("utf-8"))
            written.append(rel)

        drop_bindings(read_form, write_form, BANKS[:-len(".xml")], ["КоррСчет"])
        with open(os.path.join(out, "forms.txt"), "w", encoding="utf-8") as f:
            f.write("\n".join(sorted(written)) + "\n")
    # files.txt lists the new objects' files too
    path = os.path.join(out, "files.txt")
    with open(path, encoding="utf-8") as f:
        files = [line.strip() for line in f if line.strip()]
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(sorted(set(files) | set(tree.extra))) + "\n")
