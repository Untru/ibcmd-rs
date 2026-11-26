"""Build the compatibility-mode form fixtures with the platform: a configuration
and an extension, each with one common form, on both sides of the 8.3.19
threshold below which 8.3.27.2214 writes Form.xml without `xmlns:dcssch`.

    python make_compat_form_fixtures.py [8.3.27.2214]

Output: tests/fixtures/external/compat_forms/<case>/{input.cf|input.cfe, Form.xml}
where Form.xml is the platform's own dump of the form.
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
EXT = 'ТестРасширение'
FORM_UUID = '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a01'

FORM_MD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<CommonForm uuid="%s">
		<Properties>
			<Name>%s</Name>
			<Synonym/>
			<Comment/>
			<FormType>Managed</FormType>
			<IncludeHelpInContents>false</IncludeHelpInContents>
			<UsePurposes>
				<v8:Value xsi:type="app:ApplicationUsePurpose">PlatformApplication</v8:Value>
			</UsePurposes>
			<UseStandardCommands>false</UseStandardCommands>
			<ExtendedPresentation/>
			<Explanation/>
		</Properties>
	</CommonForm>
</MetaDataObject>'''

FORM_XML = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>
	<Events>
		<Event name="OnCreateAtServer">ПриСозданииНаСервере</Event>
	</Events>
	<ChildItems>
		<InputField name="Номер" id="1">
			<DataPath>Номер</DataPath>
			<ContextMenu name="НомерКонтекстноеМеню" id="2"/>
			<ExtendedTooltip name="НомерРасширеннаяПодсказка" id="3"/>
		</InputField>
	</ChildItems>
	<Attributes>
		<Attribute name="Номер" id="1">
			<Type>
				<v8:Type>xs:decimal</v8:Type>
				<v8:NumberQualifiers>
					<v8:Digits>10</v8:Digits>
					<v8:FractionDigits>0</v8:FractionDigits>
					<v8:AllowedSign>Nonnegative</v8:AllowedSign>
				</v8:NumberQualifiers>
			</Type>
		</Attribute>
	</Attributes>
</Form>'''

MODULE = '&НаСервере\nПроцедура ПриСозданииНаСервере(Отказ, СтандартнаяОбработка)\n\tНомер = 1;\nКонецПроцедуры\n'


def add_form(src, form, anchor, anchor_with_form):
    os.makedirs(os.path.join(src, 'CommonForms', form, 'Ext', 'Form'))
    open(os.path.join(src, 'CommonForms', form + '.xml'), 'w', encoding='utf-8').write(FORM_MD % (FORM_UUID, form))
    open(os.path.join(src, 'CommonForms', form, 'Ext', 'Form.xml'), 'w', encoding='utf-8').write(FORM_XML)
    open(os.path.join(src, 'CommonForms', form, 'Ext', 'Form', 'Module.bsl'), 'w', encoding='utf-8-sig').write(MODULE)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    assert anchor in t
    open(cfg, 'w', encoding='utf-8-sig').write(t.replace(anchor, anchor_with_form % form))


def set_mode(src, prop, mode):
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t, n = re.subn(r'<%s>[^<]*<' % prop, '<%s>%s<' % (prop, mode), t)
    assert n == 1, prop
    open(cfg, 'w', encoding='utf-8-sig').write(t)


def build(exe, work, case, mode, dst_root):
    extension = case.startswith('cfe')
    case_work = os.path.join(work, case); os.makedirs(case_work)
    log = os.path.join(case_work, 'platform.log'); ib = os.path.join(case_work, 'ib')
    src = os.path.join(case_work, 'src')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    if extension:
        form = 'ТестРасширение_Форма'
        shutil.copytree(os.path.join(SRC, EXT), src)
        add_form(src, form, '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                 '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t<CommonForm>%s</CommonForm>')
        set_mode(src, 'ConfigurationExtensionCompatibilityMode', mode)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', EXT], log)
        built = os.path.join(case_work, 'input.cfe')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpCfg', built, '-Extension', EXT], log)
        dump = os.path.join(case_work, 'dump')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    else:
        form = 'Форма'
        shutil.copytree(os.path.join(SRC, 'cfe_base'), src)
        add_form(src, form, '<Language>Русский</Language>',
                 '<Language>Русский</Language>\n\t\t\t<CommonForm>%s</CommonForm>')
        set_mode(src, 'CompatibilityMode', mode)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
        built = os.path.join(case_work, 'input.cf')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpCfg', built], log)
        dump = os.path.join(case_work, 'dump')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    dst = os.path.join(dst_root, case); os.makedirs(dst)
    shutil.copy(built, dst)
    shutil.copy(os.path.join(dump, 'CommonForms', form, 'Ext', 'Form.xml'), os.path.join(dst, 'Form.xml'))
    declared = b'xmlns:dcssch' in open(os.path.join(dst, 'Form.xml'), 'rb').read()
    print('ok', case, mode, 'dcssch declared:', declared)


def main():
    ver = sys.argv[1] if len(sys.argv) > 1 else '8.3.27.2214'
    exe = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    work = os.path.join(HERE, '.fixture-work-compat'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    dst_root = os.path.join(REPO, 'tests', 'fixtures', 'external', 'compat_forms')
    shutil.rmtree(dst_root, ignore_errors=True); os.makedirs(dst_root)
    for case, mode in [('cf_8_3_18', 'Version8_3_18'), ('cf_8_3_19', 'Version8_3_19'),
                       ('cfe_8_3_18', 'Version8_3_18'), ('cfe_8_3_19', 'Version8_3_19')]:
        build(exe, work, case, mode, dst_root)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
