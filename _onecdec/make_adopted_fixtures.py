"""Build adopted-object fixtures with the platform: a base configuration with
one common module and one catalog (object and manager modules, one form), and
an extension that adopts them with chosen controlled properties and extended
modules. Each case keeps the .cfe and the platform's own dump of the adopted
objects.

    python make_adopted_fixtures.py [case ...]      # default: every case

Output: tests/fixtures/external/adopted/<case>/{input.cfe, <dumped xml files>}
Prints each adopted row's header pairs beside the dumped properties, which is
how a property uuid is set apart from the others.
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump, v8c

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
EXT = 'ТестРасширение'
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'

NS = ('xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" '
      'xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" xmlns:v8="http://v8.1c.ru/8.1/data/core" '
      'xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" '
      'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20"')

BASE_MODULE = 'b0000000-0000-4000-8000-000000000001'
BASE_CATALOG = 'b0000000-0000-4000-8000-000000000002'
BASE_FORM = 'b0000000-0000-4000-8000-000000000003'
EXT_MODULE = 'e0000000-0000-4000-8000-000000000001'
EXT_CATALOG = 'e0000000-0000-4000-8000-000000000002'
EXT_FORM = 'e0000000-0000-4000-8000-000000000003'

MODULE_FLAGS = [('Global', 'false'), ('ClientManagedApplication', 'true'), ('Server', 'true'),
                ('ExternalConnection', 'true'), ('ClientOrdinaryApplication', 'true'), ('ServerCall', 'true'),
                ('Privileged', 'false'), ('ReturnValuesReuse', 'DontUse')]


def md(body):
    return '\ufeff<?xml version="1.0" encoding="UTF-8"?>\n<MetaDataObject %s>\n%s\n</MetaDataObject>' % (NS, body)


def generated(kind, name, seed):
    cats = [('Object', 'Object'), ('Ref', 'Ref'), ('Selection', 'Selection'), ('List', 'List'), ('Manager', 'Manager')]
    out = []
    for i, (suffix, cat) in enumerate(cats):
        out.append('\t\t\t<xr:GeneratedType name="%s%s.%s" category="%s">\n\t\t\t\t<xr:TypeId>%s-0000-4000-8000-%012d</xr:TypeId>\n'
                   '\t\t\t\t<xr:ValueId>%s-0000-4000-8000-%012d</xr:ValueId>\n\t\t\t</xr:GeneratedType>'
                   % (kind, suffix, name, cat, seed, 2 * i + 1, seed, 2 * i + 2))
    return '\n'.join(out)


def states(names):
    return ''.join('\n\t\t\t<xr:PropertyState>\n\t\t\t\t<xr:Property>%s</xr:Property>\n\t\t\t\t<xr:State>Extended</xr:State>\n\t\t\t</xr:PropertyState>' % n for n in names)


FORM_XML = ('\ufeff<?xml version="1.0" encoding="UTF-8"?>\n<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" '
            'xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">\n\t<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>\n</Form>')


BASE_EVENTS = [('OnOpen', 'ПриОткрытии'), ('OnCreateAtServer', 'ПриСозданииНаСервере'), ('BeforeClose', 'ПередЗакрытием')]
EXT_EVENTS = [('OnOpen', 'Before', 'Расш_ПриОткрытииПеред'), ('OnCreateAtServer', 'After', 'Расш_ПриСозданииНаСервереПосле'),
              ('BeforeClose', 'Override', 'Расш_ПередЗакрытиемВместо')]
# One procedure intercepting two events under different call types.
SHARED_EVENTS = [('OnOpen', 'Before', 'Расш_Общий'), ('BeforeClose', 'After', 'Расш_Общий')]


def form_with_events(events):
    lines = ''.join('\t\t<Event name="%s"%s>%s</Event>\n' % (e[0], ' callType="%s"' % e[1] if len(e) == 3 else '', e[-1]) for e in events)
    return FORM_XML.replace('</Form>', '\t<Events>\n%s\t</Events>\n</Form>' % lines)


def write(path, text, bom=False):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, 'w', encoding='utf-8-sig' if bom else 'utf-8').write(text)


def base_sources(root, case):
    shutil.copytree(os.path.join(SRC, 'cfe_base'), root)
    write(os.path.join(root, 'CommonModules', 'ОбщийМодуль.xml'), md(
        '\t<CommonModule uuid="%s">\n\t\t<Properties>\n\t\t\t<Name>ОбщийМодуль</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n%s\n\t\t</Properties>\n\t</CommonModule>'
        % (BASE_MODULE, '\n'.join('\t\t\t<%s>%s</%s>' % (k, v, k) for k, v in MODULE_FLAGS))))
    write(os.path.join(root, 'CommonModules', 'ОбщийМодуль', 'Ext', 'Module.bsl'), 'Процедура Базовая() Экспорт\nКонецПроцедуры\n', True)
    write(os.path.join(root, 'Catalogs', 'Справочник.xml'), md(
        '\t<Catalog uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<Name>Справочник</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'
        '\t\t\t<DefaultObjectForm>Catalog.Справочник.Form.ФормаЭлемента</DefaultObjectForm>\n\t\t</Properties>\n'
        '\t\t<ChildObjects>\n\t\t\t<Form>ФормаЭлемента</Form>\n\t\t</ChildObjects>\n\t</Catalog>'
        % (BASE_CATALOG, generated('Catalog', 'Справочник', 'c0000001'))))
    write(os.path.join(root, 'Catalogs', 'Справочник', 'Ext', 'ObjectModule.bsl'), 'Процедура ПередЗаписью(Отказ)\nКонецПроцедуры\n', True)
    write(os.path.join(root, 'Catalogs', 'Справочник', 'Ext', 'ManagerModule.bsl'), 'Процедура Менеджер() Экспорт\nКонецПроцедуры\n', True)
    write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента.xml'), md(
        '\t<Form uuid="%s">\n\t\t<Properties>\n\t\t\t<Name>ФормаЭлемента</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n\t\t\t<FormType>Managed</FormType>\n'
        '\t\t</Properties>\n\t</Form>' % BASE_FORM))
    if case.get('events'):
        write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента', 'Ext', 'Form.xml'), form_with_events(BASE_EVENTS))
        write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента', 'Ext', 'Form', 'Module.bsl'),
              ''.join('&НаКлиенте\nПроцедура %s(Отказ)\nКонецПроцедуры\n\n' % h for _, h in BASE_EVENTS), True)
    else:
        write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента', 'Ext', 'Form.xml'), FORM_XML)
    cfg = os.path.join(root, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<Language>Русский</Language>', '<Language>Русский</Language>\n\t\t\t<CommonModule>ОбщийМодуль</CommonModule>\n\t\t\t<Catalog>Справочник</Catalog>')
    open(cfg, 'w', encoding='utf-8-sig').write(t)


def ext_sources(root, case):
    shutil.copytree(os.path.join(SRC, EXT), root)
    children = []
    if 'module' in case:
        flags = case['module']
        st = states(['Module']) if case.get('module_extended') else ''
        info = '\t\t<InternalInfo>%s\n\t\t</InternalInfo>\n' % st if st else ''
        write(os.path.join(root, 'CommonModules', 'ОбщийМодуль.xml'), md(
            '\t<CommonModule uuid="%s">\n%s\t\t<Properties>\n\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>ОбщийМодуль</Name>\n\t\t\t<Comment/>\n'
            '\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n%s\t\t</Properties>\n\t</CommonModule>'
            % (EXT_MODULE, info, BASE_MODULE, ''.join('\t\t\t<%s>%s</%s>\n' % (k, v, k) for k, v in MODULE_FLAGS if k in flags))))
        if case.get('module_extended'):
            write(os.path.join(root, 'CommonModules', 'ОбщийМодуль', 'Ext', 'Module.bsl'), '// расширение модуля\n', True)
        children.append('<CommonModule>ОбщийМодуль</CommonModule>')
    if 'catalog' in case:
        mods = case['catalog']
        write(os.path.join(root, 'Catalogs', 'Справочник.xml'), md(
            '\t<Catalog uuid="%s">\n\t\t<InternalInfo>\n%s%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n'
            '\t\t\t<Name>Справочник</Name>\n\t\t\t<Comment/>\n\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n\t\t</Properties>\n'
            '\t\t<ChildObjects>%s\n\t\t</ChildObjects>\n\t</Catalog>'
            % (EXT_CATALOG, generated('Catalog', 'Справочник', 'c0000002'), states(mods), BASE_CATALOG,
               '\n\t\t\t<Form>ФормаЭлемента</Form>' if case.get('form') else '')))
        for m in mods:
            write(os.path.join(root, 'Catalogs', 'Справочник', 'Ext', m + '.bsl'), '// расширение %s\n' % m, True)
        if case.get('form'):
            write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента.xml'), md(
                '\t<Form uuid="%s">\n\t\t<InternalInfo>%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n'
                '\t\t\t<Name>ФормаЭлемента</Name>\n\t\t\t<Comment/>\n\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n'
                '\t\t\t<FormType>Managed</FormType>\n\t\t</Properties>\n\t</Form>' % (EXT_FORM, states(['Form']), BASE_FORM)))
            if case.get('events'):
                ext_events = SHARED_EVENTS if case.get('shared') else EXT_EVENTS
                own = form_with_events(ext_events)
                base = form_with_events(BASE_EVENTS)
                inner = base.split('version="2.20">\n', 1)[1].rsplit('</Form>', 1)[0]
                inner = ''.join('\t' + l for l in inner.splitlines(True))
                write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента', 'Ext', 'Form.xml'),
                      own.replace('</Form>', '\t<BaseForm version="2.20">\n%s\t</BaseForm>\n</Form>' % inner))
                write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента', 'Ext', 'Form', 'Module.bsl'),
                      ''.join('&НаКлиенте\nПроцедура %s(Отказ)\nКонецПроцедуры\n\n' % h
                              for h in dict.fromkeys(h for _, _, h in ext_events)), True)
            else:
                write(os.path.join(root, 'Catalogs', 'Справочник', 'Forms', 'ФормаЭлемента', 'Ext', 'Form.xml'),
                      FORM_XML.replace('</Form>', '\t<BaseForm version="2.20">\n\t\t<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>\n\t</BaseForm>\n</Form>'))
        children.append('<Catalog>Справочник</Catalog>')
    cfg = os.path.join(root, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<CommonModule>ТестРасширение_Модуль</CommonModule>',
                  '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t' + '\n\t\t\t'.join(children))
    open(cfg, 'w', encoding='utf-8-sig').write(t)


def pairs_of(cfe, uuid):
    t = v8c.read(cfe)[uuid].decode('utf-8-sig', 'replace')
    m = re.search(r'\{1,0,%s\},"[^"]*",\s*\{[^{}]*\},"[^"]*",1,(\d+),((?:\s*[0-9a-f-]{36},\d+,)*)\s*([0-9a-f-]{36})' % uuid, t)
    return re.findall(r'([0-9a-f-]{36}),(\d)', m.group(2)) if m else None


def props_of(path):
    t = open(path, encoding='utf-8-sig').read()
    body = t.split('<Properties>')[1].split('</Properties>')[0]
    return re.findall(r'^\t\t\t<(\w+)', body, flags=re.M), re.findall(r'<xr:Property>(\w+)</xr:Property>', t)


CASES = {
    # every common-module flag controlled, the module extended
    'module_all': {'module': [k for k, _ in MODULE_FLAGS], 'module_extended': True},
    # binary partitions of the eight flags: bit 0, 1, 2 of the flag index
    'module_b0': {'module': [k for i, (k, _) in enumerate(MODULE_FLAGS) if i & 1]},
    'module_b1': {'module': [k for i, (k, _) in enumerate(MODULE_FLAGS) if i & 2]},
    'module_b2': {'module': [k for i, (k, _) in enumerate(MODULE_FLAGS) if i & 4]},
    # a catalog whose object and manager modules and one form are extended
    'catalog_modules': {'catalog': ['ObjectModule', 'ManagerModule'], 'form': True},
    'catalog_object_module': {'catalog': ['ObjectModule']},
    # an adopted form intercepting three base events: before, after, instead
    'form_events': {'catalog': [], 'form': True, 'events': True},
    # one handler, two events, two call types
    'form_events_shared': {'catalog': [], 'form': True, 'events': True, 'shared': True},
}


def build(case_name, case, work, dst_root):
    w = os.path.join(work, case_name); os.makedirs(w)
    log = os.path.join(w, 'platform.log'); ib = os.path.join(w, 'ib')
    base = os.path.join(w, 'base'); base_sources(base, case)
    ext = os.path.join(w, 'ext'); ext_sources(ext, case)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(dst_root, case_name); shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    cfe = os.path.join(dst, 'input.cfe')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', cfe, '-Extension', EXT], log)
    dump = os.path.join(w, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    for rel in ['CommonModules/ОбщийМодуль.xml', 'Catalogs/Справочник.xml', 'Catalogs/Справочник/Forms/ФормаЭлемента.xml',
                'Catalogs/Справочник/Forms/ФормаЭлемента/Ext/Form.xml']:
        src = os.path.join(dump, rel)
        if os.path.exists(src):
            os.makedirs(os.path.dirname(os.path.join(dst, rel)), exist_ok=True)
            shutil.copy(src, os.path.join(dst, rel))
            if rel.endswith('Form.xml'):
                continue
            uuid = re.search(r'uuid="([^"]+)"', open(src, encoding='utf-8-sig').read()).group(1)
            print(case_name, rel, 'pairs', pairs_of(cfe, uuid), 'props/states', props_of(src))


def main():
    names = sys.argv[1:] or list(CASES)
    work = os.path.join(HERE, '.fixture-work-adopted'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    dst_root = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted')
    os.makedirs(dst_root, exist_ok=True)
    for name in names:
        build(name, CASES[name], work, dst_root)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
