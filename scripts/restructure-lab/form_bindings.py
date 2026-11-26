"""Drops the bindings of the forms of an object to the attributes a stage removes (S1 combinations, the route through our import).

A tree that removes an attribute of a catalog or a document but leaves the fields of its forms bound to it (`Объект.<attribute>` in the item
form, `Список.<attribute>` in a list) cannot be a configuration: the platform keeps such a binding as broken (`~Список.X`) and our import refuses
to stage a tree it cannot reproduce. The edit a user makes is to take the fields out of the forms; this does it, on the text of the export:
every element whose own `<DataPath>` is `Объект.<attribute>` or `Список.<attribute>` (with the element's whole subtree) is removed.

usage as a module: drop_bindings(read, write, object_dir, names) with read(rel) -> text and write(rel, text); object_dir like `Catalogs/КлассификаторБанков`.
"""
import os
import re

from edit_cases_s1 import REF, nl_of


def forms_of(object_dir):
    root = os.path.join(REF, object_dir, "Forms")
    if not os.path.isdir(root):
        return []
    return ["%s/Forms/%s/Ext/Form.xml" % (object_dir, name) for name in sorted(os.listdir(root))
            if os.path.exists(os.path.join(root, name, "Ext", "Form.xml"))]


def drop_paths(text, paths):
    """`text` without the elements bound to one of `paths`; the number of elements dropped."""
    nl = nl_of(text)
    lines = text.split(nl)
    dropped = 0
    while True:
        at = None
        for i, line in enumerate(lines):
            stripped = line.strip()
            if stripped.startswith("<DataPath>") and stripped.endswith("</DataPath>") and stripped[len("<DataPath>"):-len("</DataPath>")] in paths:
                at = i
                break
        if at is None:
            return nl.join(lines), dropped
        indent = len(lines[at]) - len(lines[at].lstrip("\t"))
        start = at
        while start >= 0:
            line = lines[start]
            depth = len(line) - len(line.lstrip("\t"))
            if depth == indent - 1 and line.lstrip("\t").startswith("<") and not line.lstrip("\t").startswith("</"):
                break
            start -= 1
        assert start >= 0, "no element around the binding at line %d" % at
        tag = lines[start].lstrip("\t").split()[0].lstrip("<").rstrip(">")
        end = at
        while end < len(lines):
            if len(lines[end]) - len(lines[end].lstrip("\t")) == indent - 1 and lines[end].lstrip("\t") == "</%s>" % tag:
                break
            end += 1
        assert end < len(lines), "no end of <%s> at line %d" % (tag, start)
        del lines[start:end + 1]
        dropped += 1


def drop_query_columns(text, names):
    """`text` without the columns of a manual query (`Таблица.<name> КАК <name>,`) that read a removed attribute; the number dropped.
    A list form of a catalog keeps the query text: a query that reads a column that is gone would break every field of the list.
    (The lines inside the query text end with a bare line feed, the lines of the form with the file's own.)"""
    dropped = 0
    for name in names:
        column = r"Таблица\.%s КАК %s" % (re.escape(name), re.escape(name))
        text, middle = re.subn(r"\n[ \t]*%s,(?=\r?\n)" % column, "", text)
        text, last = re.subn(r",\n[ \t]*%s(?=\r?\n|</QueryText>)" % column, "", text)
        dropped += middle + last
    return text, dropped


def drop_bindings(read, write, object_dir, names):
    total = 0
    for rel in forms_of(object_dir):
        text = read(rel)
        paths = set()
        for name in names:
            paths.add("Объект.%s" % name)
            paths.add("Список.%s" % name)
        new, dropped = drop_paths(text, paths)
        new, columns = drop_query_columns(new, names)
        if dropped or columns:
            write(rel, new)
            total += dropped + columns
            print("  form %s: %d element(s) bound to %s dropped, %d query column(s)" % (rel, dropped, ", ".join(sorted(names)), columns))
    return total
