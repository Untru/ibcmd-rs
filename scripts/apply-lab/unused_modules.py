#!/usr/bin/env python3
"""Common modules of an exported tree that no XML file refers to (candidates for a removal edit without side effects).

usage: unused_modules.py <tree>
A module is unused when no XML file other than Configuration.xml, ConfigDumpInfo.xml and its own files names
`CommonModule.<name>`; the roles' rights files are included (a right on a module is written as `CommonModule.<name>`).
"""
import os
import re
import sys

sys.stdout.reconfigure(encoding='utf-8')
tree = sys.argv[1]
pattern = re.compile(r'CommonModule\.([^\s<>".,;:()]+)')
mentioned = {}
for base, _, files in os.walk(tree):
    for name in files:
        if not name.endswith('.xml') or name == 'ConfigDumpInfo.xml':
            continue
        path = os.path.join(base, name)
        rel = os.path.relpath(path, tree).replace('\\', '/')
        with open(path, 'rb') as handle:
            text = handle.read().decode('utf-8', 'replace')
        for hit in set(pattern.findall(text)):
            mentioned.setdefault(hit, set()).add(rel)
modules = sorted(name[:-4] for name in os.listdir(os.path.join(tree, 'CommonModules')) if name.endswith('.xml'))
for module in modules:
    own = {'CommonModules/%s.xml' % module}
    users = mentioned.get(module, set()) - own
    if not users:
        size = 0
        folder = os.path.join(tree, 'CommonModules', module)
        for base, _, files in os.walk(folder):
            size += sum(os.path.getsize(os.path.join(base, f)) for f in files)
        print('%-60s %8d bytes' % (module, size))
