"""Build the adopted-kinds fixture with the platform: a base configuration
with a chart of characteristic types, a task and a business process, and an
extension adopting each with nothing but the object itself controlled.

    python make_adopted_kinds_fixture.py

Output: tests/fixtures/external/adopted/kinds/{input.cfe, <dumped xml>}
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted
import make_adopted_properties_fixture as props

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT, write, md = adopted.EXE, adopted.EXT, adopted.write, props.md


def uid(side, n):
    return '%s6000000-0000-4000-8000-%012d' % (side, n)


def gen(items, seed):
    return '\n'.join(
        '\t\t\t<xr:GeneratedType name="%s" category="%s">\n\t\t\t\t<xr:TypeId>%s-0000-4000-8000-%012d</xr:TypeId>\n'
        '\t\t\t\t<xr:ValueId>%s-0000-4000-8000-%012d</xr:ValueId>\n\t\t\t</xr:GeneratedType>'
        % (n, c, seed, 2 * i + 1, seed, 2 * i + 2) for i, (n, c) in enumerate(items))


KINDS = {
    'ChartOfCharacteristicTypes': ('ChartsOfCharacteristicTypes', 'ПВХ', [
        ('ChartOfCharacteristicTypesObject', 'Object'), ('ChartOfCharacteristicTypesRef', 'Ref'),
        ('ChartOfCharacteristicTypesSelection', 'Selection'), ('ChartOfCharacteristicTypesList', 'List'),
        ('Characteristic', 'Characteristic'), ('ChartOfCharacteristicTypesManager', 'Manager')]),
    'Task': ('Tasks', 'Задача', [
        ('TaskObject', 'Object'), ('TaskRef', 'Ref'), ('TaskSelection', 'Selection'), ('TaskList', 'List'),
        ('TaskManager', 'Manager')]),
    'BusinessProcess': ('BusinessProcesses', 'Процесс', [
        ('BusinessProcessObject', 'Object'), ('BusinessProcessRef', 'Ref'), ('BusinessProcessSelection', 'Selection'),
        ('BusinessProcessList', 'List'), ('BusinessProcessManager', 'Manager'),
        ('BusinessProcessRoutePointRef', 'RoutePointRef')]),
}
BASE_PROPS = {
    'ChartOfCharacteristicTypes': '\t\t\t<Type>\n\t\t\t\t<v8:Type>xs:boolean</v8:Type>\n\t\t\t</Type>\n',
    'Task': '',
    'BusinessProcess': '\t\t\t<Task>Task.Задача</Task>\n',
}


def obj(kind, seed, side, adopt_from=None):
    folder, name, cats = KINDS[kind]
    items = [('%s.%s' % (p, name), c) for p, c in cats]
    if adopt_from:
        head = ('\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>%s</Name>\n\t\t\t<Comment/>\n'
                '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n' % (name, adopt_from))
    else:
        head = '\t\t\t<Name>%s</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n%s' % (name, BASE_PROPS[kind])
    return md('\t<%s uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n%s\t\t</Properties>\n'
              '\t\t<ChildObjects/>\n\t</%s>' % (kind, uid(side, seed), gen(items, '%s600000%d' % ('c' if side == 'b' else 'd', seed)), head, kind))


def main():
    work = tempfile.mkdtemp(prefix='adopted-kinds-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    names = []
    for seed, kind in enumerate(KINDS, 1):
        folder, name, _ = KINDS[kind]
        write(os.path.join(base, folder, name + '.xml'), obj(kind, seed, 'b'))
        write(os.path.join(ext, folder, name + '.xml'), obj(kind, seed, 'e', uid('b', seed)))
        names.append((kind, name))
    props.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>', names)
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>', names)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'kinds')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    for kind in KINDS:
        folder, name, _ = KINDS[kind]
        rel = os.path.join(folder, name + '.xml')
        os.makedirs(os.path.join(dst, folder), exist_ok=True)
        shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
        print(rel, open(os.path.join(dump, rel), encoding='utf-8-sig').read().split('<Properties>')[1][:600])
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
