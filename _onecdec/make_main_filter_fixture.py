"""Build the main-filter fixture with the platform: a recorder-subordinate
periodic information register loaded with MainFilterOnPeriod true, in a
configuration and in an extension. The stored flag stays set; the dump says
false.

    python make_main_filter_fixture.py

Output: tests/fixtures/external/main_filter/{cf,cfe}/{input.*, InformationRegisters/*.xml}
"""
import os, re, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
ON = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ON)
import v8dump
import make_adopted_fixtures as A
import make_adopted_properties_fixture as P

EXE = A.EXE


def reg(name, uuid, seed):
    cats = [('InformationRegister' + a, a) for a in ['Record', 'Manager', 'Selection', 'List', 'RecordSet', 'RecordKey', 'RecordManager']]
    info = P.generated('InformationRegister', name, seed, cats)
    return P.md('\t<InformationRegister uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<Name>%s</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'
                '\t\t\t<InformationRegisterPeriodicity>Second</InformationRegisterPeriodicity>\n\t\t\t<WriteMode>RecorderSubordinate</WriteMode>\n'
                '\t\t\t<MainFilterOnPeriod>true</MainFilterOnPeriod>\n\t\t</Properties>\n\t\t<ChildObjects>\n%s\n\t\t</ChildObjects>\n\t</InformationRegister>'
                % (uuid, info, name, P.resource(uuid[:-3] + '999', 'Resource', 'Ресурс', P.NUMBER10)))


def doc(name, uuid, seed, reg_name):
    cats = [('Document' + a, a) for a in ['Object', 'Ref', 'Selection', 'List', 'Manager']]
    return P.md('\t<Document uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<Name>%s</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'
                '\t\t\t<RegisterRecords>\n\t\t\t\t<xr:Item xsi:type="xr:MDObjectRef">InformationRegister.%s</xr:Item>\n\t\t\t</RegisterRecords>\n'
                '\t\t</Properties>\n\t\t<ChildObjects/>\n\t</Document>' % (uuid, P.generated('Document', name, seed, cats), name, reg_name))


work = tempfile.mkdtemp(prefix='probe-mfp-')
log = os.path.join(work, 'log'); ib = os.path.join(work, 'ib')
base = os.path.join(work, 'base'); shutil.copytree(os.path.join(A.SRC, 'cfe_base'), base)
ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(A.SRC, A.EXT), ext)
A.write(os.path.join(base, 'InformationRegisters', 'Рег.xml'), reg('Рег', 'b7000000-0000-4000-8000-000000000001', 'c7000001'))
A.write(os.path.join(base, 'Documents', 'Док.xml'), doc('Док', 'b7000000-0000-4000-8000-000000000002', 'c7000002', 'Рег'))
P.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>', [('InformationRegister', 'Рег'), ('Document', 'Док')])
E = 'ТестРасширение_'
A.write(os.path.join(ext, 'InformationRegisters', E + 'Рег.xml'), reg(E + 'Рег', 'e7000000-0000-4000-8000-000000000001', 'd7000001'))
A.write(os.path.join(ext, 'Documents', E + 'Док.xml'), doc(E + 'Док', 'e7000000-0000-4000-8000-000000000002', 'd7000002', E + 'Рег'))
P.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>', [('InformationRegister', E + 'Рег'), ('Document', E + 'Док')])
v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', A.EXT], log)
d1 = os.path.join(work, 'd1'); d2 = os.path.join(work, 'd2')
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', d1], log)
v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', d2, '-Extension', A.EXT], log)
root = os.path.join(os.path.dirname(ON), 'tests', 'fixtures', 'external', 'main_filter')
shutil.rmtree(root, ignore_errors=True)
for case, dump, name, args in [('cf', d1, 'Рег', []), ('cfe', d2, E + 'Рег', ['-Extension', A.EXT])]:
    dst = os.path.join(root, case)
    os.makedirs(os.path.join(dst, 'InformationRegisters'))
    rel = os.path.join('InformationRegisters', name + '.xml')
    shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.' + case)] + args, log)
    print(case, re.findall(r'<MainFilterOnPeriod>\w+', open(os.path.join(dst, rel), encoding='utf-8-sig').read()))
shutil.rmtree(work, ignore_errors=True)
