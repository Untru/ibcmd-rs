#!/usr/bin/env python3
"""Removal edits of a working copy of the exported БСП 8.3.27 tree (#393), in the style of the import lab's edits.py
(scripts/import-lab, not modified): formdel is used from there; the others are here.

usage: tree_edit.py <work tree> <edit>[,<edit>...]      (the work tree is a full copy; the base is never written)
  tpldel   the text template ДатыПасха removed from DataProcessor.ЗаполнениеКалендарныхГрафиков (files and the <Template> entry)
  moddel   the manager module of DataProcessor.ЗаполнениеКалендарныхГрафиков removed (Ext/ManagerModule.bsl)
  cmdel    the common module _ДемоЗаметки removed (files and the <CommonModule> entry of Configuration.xml)
Trees are UTF-8 with BOM and CRLF; the edits keep that (bytes are replaced, not re-encoded).
"""
import os
import shutil
import sys

sys.stdout.reconfigure(encoding='utf-8')
work = sys.argv[1]
edits = sys.argv[2].split(',')


def path(rel):
    return os.path.join(work, rel.replace('/', os.sep))


def drop_line(rel, line):
    p = path(rel)
    data = open(p, 'rb').read()
    needle = (line + '\r\n').encode('utf-8')
    assert data.count(needle) == 1, (rel, line, data.count(needle))
    open(p, 'wb').write(data.replace(needle, b'', 1))


def remove(rel):
    p = path(rel)
    if os.path.isdir(p):
        shutil.rmtree(p)
    elif os.path.exists(p):
        os.remove(p)
    else:
        raise SystemExit('no such file: ' + rel)


for edit in edits:
    if edit == 'tpldel':
        owner = 'DataProcessors/ЗаполнениеКалендарныхГрафиков'
        remove(owner + '/Templates/ДатыПасха.xml')
        remove(owner + '/Templates/ДатыПасха')
        drop_line(owner + '.xml', '\t\t\t<Template>ДатыПасха</Template>')
    elif edit == 'moddel':
        owner = 'DataProcessors/ЗаполнениеКалендарныхГрафиков'
        remove(owner + '/Ext/ManagerModule.bsl')
    elif edit == 'cmdel':
        remove('CommonModules/_ДемоЗаметки.xml')
        remove('CommonModules/_ДемоЗаметки')
        drop_line('Configuration.xml', '\t\t\t<CommonModule>_ДемоЗаметки</CommonModule>')
    else:
        raise SystemExit('unknown edit ' + edit)
    print('applied', edit)
