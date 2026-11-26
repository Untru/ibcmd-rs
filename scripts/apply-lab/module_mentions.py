#!/usr/bin/env python3
"""Which kinds of files mention the common modules of a tree: a histogram of the sets of top-level folders.

usage: module_mentions.py <tree>
"""
import collections
import os
import re
import sys

sys.stdout.reconfigure(encoding='utf-8')
tree = sys.argv[1]
pattern = re.compile(r'CommonModule\.([^\s<>".,;:()]+)')
where = collections.defaultdict(collections.Counter)
for base, _, files in os.walk(tree):
    for name in files:
        if not name.endswith('.xml') or name == 'ConfigDumpInfo.xml':
            continue
        path = os.path.join(base, name)
        rel = os.path.relpath(path, tree).replace(os.sep, '/')
        text = open(path, 'rb').read().decode('utf-8', 'replace')
        for hit in set(pattern.findall(text)):
            where[hit][rel.split('/')[0]] += 1
histogram = collections.Counter(tuple(sorted(folders)) for folders in where.values())
print('modules mentioned:', len(where))
for kinds, count in histogram.most_common(10):
    print(count, kinds)
