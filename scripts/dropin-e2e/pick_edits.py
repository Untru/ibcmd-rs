"""Picks the files of the reference tree the end-to-end edit touches, and writes them as a manifest.

usage: python pick_edits.py <manifest.json>
For every wanted (label, top folder, file pattern) the first file (sorted) of a `_Демо*` object that exists and is not
nearly empty is taken. The manifest lists label, relative path, edit kind.
"""
import json
import os
import re
import sys

REF = r'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native'

# label, top folder, path regex (relative to the top folder, with / as the separator), edit kind, minimum size
WANTED = [
    ('CommonModule', 'CommonModules', r'^_ДемоЛокализация/Ext/Module\.bsl$', 'bsl', 200),
    ('CommonModule, updated online in the corpus', 'CommonModules', r'^_ДемоЗаметки/Ext/Module\.bsl$', 'bsl', 200),
    ('Catalog object module', 'Catalogs', r'^_Демо[^/]+/Ext/ObjectModule\.bsl$', 'bsl', 400),
    ('Catalog manager module', 'Catalogs', r'^_Демо[^/]+/Ext/ManagerModule\.bsl$', 'bsl', 400),
    ('Catalog form module', 'Catalogs', r'^_Демо[^/]+/Forms/[^/]+/Ext/Form/Module\.bsl$', 'bsl', 400),
    ('Catalog form', 'Catalogs', r'^_Демо[^/]+/Forms/[^/]+/Ext/Form\.xml$', 'form_xml', 800),
    ('Catalog template (spreadsheet)', 'Catalogs', r'^_Демо[^/]+/Templates/[^/]+/Ext/Template\.xml$', 'template_xml', 800),
    ('Catalog help', 'Catalogs', r'^_Демо[^/]+/Ext/Help/ru\.html$', 'help_html', 800),
    ('Document object module', 'Documents', r'^_ДемоЗаказПокупателя/Ext/ObjectModule\.bsl$', 'bsl', 400),
    ('Document manager module', 'Documents', r'^_ДемоЗаказПокупателя/Ext/ManagerModule\.bsl$', 'bsl', 400),
    ('Document form module', 'Documents', r'^_Демо[^/]+/Forms/[^/]+/Ext/Form/Module\.bsl$', 'bsl', 400),
    ('Document template (spreadsheet)', 'Documents', r'^_ДемоПеремещениеТоваров/Templates/[^/]+/Ext/Template\.xml$', 'template_xml', 800),
    ('Document template (binary)', 'Documents', r'^_Демо[^/]+/Templates/[^/]+/Ext/Template\.bin$', 'template_bin', 200),
    ('DataProcessor object module', 'DataProcessors', r'^_Демо[^/]+/Ext/ObjectModule\.bsl$', 'bsl', 400),
    ('DataProcessor form module', 'DataProcessors', r'^_Демо[^/]+/Forms/[^/]+/Ext/Form/Module\.bsl$', 'bsl', 400),
    ('DataProcessor form', 'DataProcessors', r'^_Демо[^/]+/Forms/[^/]+/Ext/Form\.xml$', 'form_xml', 800),
    ('Report manager module', 'Reports', r'^_Демо[^/]+/Ext/ManagerModule\.bsl$', 'bsl', 400),
    ('Report template (data composition schema)', 'Reports', r'^_Демо[^/]+/Templates/[^/]+/Ext/Template\.xml$', 'template_xml', 800),
    ('InformationRegister manager module', 'InformationRegisters', r'^_Демо[^/]+/Ext/ManagerModule\.bsl$', 'bsl', 400),
    ('InformationRegister record set module', 'InformationRegisters', r'^_Демо[^/]+/Ext/RecordSetModule\.bsl$', 'bsl', 400),
    ('Constant value manager module', 'Constants', r'^_Демо[^/]+/Ext/ValueManagerModule\.bsl$', 'bsl', 200),
    ('CommonCommand module', 'CommonCommands', r'^_Демо[^/]+/Ext/CommandModule\.bsl$', 'bsl', 100),
    ('CommonForm module', 'CommonForms', r'^_ДемоМоиНастройки/Ext/Form/Module\.bsl$', 'bsl', 400),
    ('CommonForm', 'CommonForms', r'^_ДемоМоиНастройки/Ext/Form\.xml$', 'form_xml', 800),
    ('CommonTemplate (binary)', 'CommonTemplates', r'^_ДемоПФ_ODT_СчетНаОплату_ru/Ext/Template\.bin$', 'template_bin', 200),
    ('ExchangePlan object module', 'ExchangePlans', r'^_Демо[^/]+/Ext/ObjectModule\.bsl$', 'bsl', 400),
    ('BusinessProcess object module', 'BusinessProcesses', r'^_Демо[^/]+/Ext/ObjectModule\.bsl$', 'bsl', 400),
]


def main():
    picked = []
    used_objects = set()
    for label, top, pattern, kind, minimum in WANTED:
        base = os.path.join(REF, top)
        found = None
        candidates = []
        for root, dirs, files in os.walk(base):
            dirs.sort()
            for name in sorted(files):
                full = os.path.join(root, name)
                rel = os.path.relpath(full, base).replace(os.sep, '/')
                if re.match(pattern, rel):
                    candidates.append((rel, full))
        candidates.sort()
        for rel, full in candidates:
            if os.path.getsize(full) < minimum:
                continue
            object_name = top + '/' + rel.split('/')[0]
            found = (rel, full, object_name)
            break
        if not found:
            print('NOT FOUND:', label, file=sys.stderr)
            continue
        rel, full, object_name = found
        picked.append({'label': label, 'path': top + '/' + rel, 'kind': kind, 'size': os.path.getsize(full)})
    with open(sys.argv[1], 'w', encoding='utf-8', newline='\n') as f:
        json.dump(picked, f, ensure_ascii=False, indent=1)
    print(len(picked), 'files picked')


if __name__ == '__main__':
    main()
