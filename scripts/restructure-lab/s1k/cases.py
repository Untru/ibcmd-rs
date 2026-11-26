"""The cases of S1-K (#407): an edit of the БСП 8.3.27 tree, then OUR import and OUR apply against the native ones.

usage:
  python cases.py list
  python cases.py edit <case> <out dir>     writes <out>/stage/<rel> (the edited files), <out>/before/<rel> (the
                                            originals) and <out>/files.txt; the reference tree is never written
  python cases.py files <out dir>           the relative paths of the edited files, one per line

Every editor is the ddl track's (`edit_cases_s1.py`, `edit_cases_s2.py`) or a small one of this kit; the edits are
the ones the S1 operations were traced and twin-verified with, so a difference between our chain and the native one is
the chain's, not the edit's. `REF` is the reference native export of the БСП 8.3.27 corpus clone (12 198 files).
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import edit_cases_s1 as s1  # noqa: E402
import edit_cases_s2 as s2  # noqa: E402
import edit_cases_s3 as s3  # noqa: E402
import new_objects  # noqa: E402

REF = s1.REF
EXT_TREES = os.environ.get("S1K_EXT_TREES", r"F:\ibcmd\lab\05\ext\native\8327")
# The objects the four БСП extensions adopt (a fallback for a machine without the reference trees of the ext track):
# an object in this list is refused by S1-I, so the cases of the built operations avoid them.
ADOPTED_FALLBACK = {
    "Catalogs/_ДемоГруппыДоступаПартнеров.xml", "Catalogs/_ДемоКонтрагенты.xml", "Catalogs/_ДемоМестаХранения.xml",
    "Catalogs/_ДемоНоменклатура.xml", "Catalogs/_ДемоОрганизации.xml", "Catalogs/_ДемоПартнеры.xml",
    "Catalogs/ВидыКонтактнойИнформации.xml", "Catalogs/Пользователи.xml",
    "Documents/_ДемоПоступлениеТоваров.xml", "Documents/_ДемоРеализацияТоваров.xml",
    "Documents/_ДемоСписаниеТоваров.xml", "Documents/_ДемоСчетНаОплатуПокупателю.xml",
}


def adopted_files():
    """Relative paths (Catalogs/X.xml) of the objects any extension tree of the ext track adopts."""
    found = set()
    if os.path.isdir(EXT_TREES):
        for extension in os.listdir(EXT_TREES):
            for kind in ("Catalogs", "Documents"):
                folder = os.path.join(EXT_TREES, extension, kind)
                if not os.path.isdir(folder):
                    continue
                for name in os.listdir(folder):
                    if name.endswith(".xml"):
                        path = os.path.join(folder, name)
                        with open(path, encoding="utf-8-sig") as f:
                            if "<ObjectBelonging>Adopted</ObjectBelonging>" in f.read(6000):
                                found.add(kind + "/" + name)
    return found or ADOPTED_FALLBACK


def run_a1(out):
    """A: attributes of every primitive type on the objects no extension adopts (the types case of the ddl track
    without its three adopted catalogs)."""
    adopted = adopted_files()
    tree = s1.Tree(out, "a1")
    files = {}
    for rel, position, attrs in s1.T1:
        if rel in adopted:
            continue
        text, bom = files[rel] if rel in files else tree.read(rel)
        nl = s1.nl_of(text)
        is_document = rel.startswith("Documents/")
        for name, synonym, type_xml, use in attrs:
            block = s1.attribute_block(name, synonym, type_xml, nl, use=None if is_document else use)
            if position == "end":
                text = s1.insert_after_last_attribute(text, block, nl)
            elif position == "first":
                text = s1.add_first_attribute(text, block, nl)
            else:
                text = s1.insert_before_attribute(text, position[1], block, nl)
        files[rel] = (text, bom)
    for rel, (text, bom) in files.items():
        tree.write(rel, text, bom)
        print("edited", rel)
    with open(os.path.join(out, "files.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(sorted(files)) + "\n")


def run_i1(out):
    """I: a string attribute on two objects the extension `_ДемоРасширение` adopts, one of them with a table of the
    extension's own (`_Reference18X1`): S1-I refuses both."""
    tree = s2.Tree(out)
    s2.append_attribute(tree, "Catalogs/_ДемоПартнеры.xml", "ДемоНовыйРеквизитСИ", "Демо новый реквизит СИ", s1.string_var(50), "ForItem")
    s2.append_attribute(tree, "Catalogs/_ДемоНоменклатура.xml", "ДемоНовыйРеквизитСИ", "Демо новый реквизит СИ", s1.string_var(50), "ForItem")
    tree.save()


def unbind_form(text, attr):
    """The form text without what binds the deleted attribute `attr` of the object: the items whose `<DataPath>` is
    `Объект.<attr>` / `Список.<attr>` (with their menus, tooltips and children), the `<Field>Список.<attr></Field>`
    lines of a dynamic list, and the field of a manual query text (`<alias>.<attr>,`). Returns (text, removed)."""
    lines = text.split("\n")
    removed = []

    def indent(line):
        return len(line) - len(line.lstrip("\t"))

    # items
    i = 0
    while i < len(lines):
        stripped = lines[i].strip()
        # the item that binds the attribute: a form item by its data path, or an item of the list's settings (an
        # order, a filter, a selected field) by its field
        if stripped in (
            "<DataPath>Объект.%s</DataPath>" % attr,
            "<DataPath>Список.%s</DataPath>" % attr,
            "<dcsset:field>%s</dcsset:field>" % attr,
            '<dcsset:left xsi:type="dcscor:Field">%s</dcsset:left>' % attr,
        ):
            depth = indent(lines[i]) - 1
            start = i
            while start > 0 and not (indent(lines[start]) == depth and lines[start].lstrip("\t").startswith("<") and not lines[start].lstrip("\t").startswith("</")):
                start -= 1
            end = i
            while end < len(lines) and not (indent(lines[end]) == depth and lines[end].lstrip("\t").startswith("</")):
                end += 1
            removed.append("item " + lines[start].strip()[:60])
            del lines[start:end + 1]
            i = start
            continue
        i += 1
    # the fields a dynamic list keeps always selected
    kept = []
    for line in lines:
        if line.strip() == "<Field>Список.%s</Field>" % attr:
            removed.append("field " + attr)
            continue
        kept.append(line)
    lines = kept
    # a manual query: `<alias>.<attr>,` (the last field has no comma: the one before loses its own)
    field = re.compile(r"^\t+[^\s<>.]+\.%s(,?)\r?$" % re.escape(attr))
    out = []
    for line in lines:
        match = field.match(line)
        if match:
            removed.append("query " + line.strip())
            if not match.group(1) and out and out[-1].rstrip("\r").endswith(","):
                out[-1] = out[-1].rstrip("\r")[:-1] + ("\r" if out[-1].endswith("\r") else "")
            continue
        out.append(line)
    return "\n".join(out), removed


def form_paths(rel_object):
    """The Form.xml files of an object of the reference tree: Catalogs/X -> [Catalogs/X/Forms/F/Ext/Form.xml]."""
    folder = os.path.join(REF, rel_object, "Forms")
    if not os.path.isdir(folder):
        return []
    return [
        "%s/Forms/%s/Ext/Form.xml" % (rel_object, name)
        for name in sorted(os.listdir(folder))
        if os.path.isfile(os.path.join(folder, name, "Ext", "Form.xml"))
    ]


def with_form_unbinding(name):
    """The s2 case `name` (deleted attributes) with the bindings of the deleted attributes taken out of the object's
    forms: the platform refuses a tree whose form binds an attribute the object no longer has, and a case of our
    chain must fail (or not) for the operation, not for the edit."""
    def run(out):
        tree = s2.Tree(out)
        deleted = []
        original = s2.remove_attribute

        def recording(t, rel, attribute):
            deleted.append((rel[:-len(".xml")], attribute))
            original(t, rel, attribute)

        s2.remove_attribute = recording
        try:
            s2.CASES[name](tree)
        finally:
            s2.remove_attribute = original
        for obj, attribute in deleted:
            for form in form_paths(obj):
                # a form the deletion does not touch stays out of the stage
                if form in tree.files:
                    current = tree.files[form][0]
                else:
                    with open(os.path.join(REF, form), encoding="utf-8-sig") as f:
                        current = f.read()
                text, removed = unbind_form(current, attribute)
                if removed:
                    tree.put(form, text)
                    current = text
                    print("unbound %s from %s: %s" % (attribute, form, "; ".join(removed)))
                left = re.findall(r"[^\n]*\b%s\b[^\n]*" % re.escape(attribute), current)
                if left:
                    print("WARNING %s still mentions %s: %s" % (form, attribute, left[0].strip()[:100]))
        tree.save()
    return run


def from_s2(name):
    def run(out):
        tree = s2.Tree(out)
        s2.CASES[name](tree)
        tree.save()
    return run


def from_s3(name):
    def run(out):
        tree = s3.Tree(out)
        s3.CASES[name](tree)
        tree.save()
    return run


# id -> (S1 operation, title, editor, built, expected outcome of the chain with the import of this branch, notes)
CASES = {
    "a1": ("A", "add attributes of every primitive type (objects no extension adopts)", run_a1, True),
    "b1": ("B", "delete attributes (middle, last, first, indexed, only, hierarchical, document, delete + add)", with_form_unbinding("b1"), True),
    "b2": ("B", "delete an attribute with the additional-order index (catalog, document)", with_form_unbinding("b2"), True),
    "c1": ("C", "widen variable strings (catalog, hierarchical, document, indexed, up to 1024)", from_s2("c1"), True),
    "d0": ("D", "the index flag: one alone", from_s2("d0"), True),
    "d1": ("D", "the index flag on and off, additional order", from_s2("d1"), True),
    "i1": ("I", "an attribute on adopted objects (refused by S1-I)", run_i1, True),
    # designed in docs/apply/restructuring.md 12.3, not built in the gate yet: the editors come with the operations
    # S1-E (merged): the shapes of the ddl track's e1 and e3 on the objects no extension adopts
    "e5": ("E", "add tabular sections (flat, hierarchical and subordinate catalogs, two documents)", from_s3("e5"), True),
    "e6": ("E", "add attributes to existing tabular sections (first / middle / last, indexed; catalog and document)", from_s3("e6"), True),
    # S1-F (merged): a new object with attributes; no forms, templates, commands or predefined items, one new object per kind per stage
    "f1": ("F", "add a flat catalog with attributes of every primitive type, a reference, an indexed one (trace's N1)", new_objects.run_f1, True),
    "f2": ("F", "add a document with a periodic numeric number and indexed attributes (trace's N4)", new_objects.run_f2, True),
}

if __name__ == "__main__":
    command = sys.argv[1] if len(sys.argv) > 1 else "list"
    if command == "list":
        for case, (operation, title, editor, built) in CASES.items():
            print("%s\t%s\t%s\t%s" % (case, operation, "built" if built else "pending", title))
    elif command == "edit":
        case, out = sys.argv[2], sys.argv[3]
        operation, title, editor, built = CASES[case]
        if editor is None:
            raise SystemExit("case %s (%s) has no editor: the operation is not built yet" % (case, title))
        os.makedirs(out, exist_ok=True)
        if os.path.exists(os.path.join(out, "new.txt")):
            os.remove(os.path.join(out, "new.txt"))   # the files a case adds are listed by its editor; an old list is not this case's
        editor(out)
    elif command == "files":
        with open(os.path.join(sys.argv[2], "files.txt"), encoding="utf-8") as f:
            print(f.read().strip())
    else:
        raise SystemExit("unknown command " + command)
