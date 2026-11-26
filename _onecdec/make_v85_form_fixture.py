"""Build the 8.5-saved form fixture: an extension with one common form, saved by
8.5.1.1529 (an 8.5 form body, `{59,…}`) and dumped by 8.3.27.2214 -- how
8.3.27 reads an 8.5 body, without the properties only 8.5 has.

    python make_v85_form_fixture.py

Output: tests/fixtures/external/v85_form/{input.cfe, Form.xml, Report.xml}
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
EXT = 'ТестРасширение'
FORM = 'ТестРасширение_Форма85'
EXE27 = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
EXE85 = r'C:\Program Files\1cv8\8.5.1.1529\bin\1cv8.exe'

FORM_MD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<CommonForm uuid="6c1f0b2e-3a4d-4e5f-8a9b-0c1d2e3f4a5b">
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
</MetaDataObject>''' % FORM

FORM_XML = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>
	<CommandBarLocation>Bottom</CommandBarLocation>
	<AutoCommandBar name="ФормаКоманднаяПанель" id="-1">
		<HorizontalAlign>Right</HorizontalAlign>
		<Autofill>false</Autofill>
	</AutoCommandBar>
	<ChildItems>
		<Button name="Выполнить" id="1">
			<Type>UsualButton</Type>
			<DefaultButton>true</DefaultButton>
			<CommandName>Form.Command.Выполнить</CommandName>
			<ExtendedTooltip name="ВыполнитьРасширеннаяПодсказка" id="2"/>
		</Button>
	</ChildItems>
	<Commands>
		<Command name="Выполнить" id="1">
			<Action>Выполнить</Action>
		</Command>
	</Commands>
</Form>'''

REPORT = 'ТестРасширение_Отчет'
REPORT_MD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<Report uuid="7d2e1c3b-4a5f-4b6c-9d8e-1f2a3b4c5d6e">
		<InternalInfo>
			<xr:GeneratedType name="ReportObject.%s" category="Object">
				<xr:TypeId>7d2e1c3b-0000-4000-8000-000000000001</xr:TypeId>
				<xr:ValueId>7d2e1c3b-0000-4000-8000-000000000002</xr:ValueId>
			</xr:GeneratedType>
			<xr:GeneratedType name="ReportManager.%s" category="Manager">
				<xr:TypeId>7d2e1c3b-0000-4000-8000-000000000003</xr:TypeId>
				<xr:ValueId>7d2e1c3b-0000-4000-8000-000000000004</xr:ValueId>
			</xr:GeneratedType>
		</InternalInfo>
		<Properties>
			<Name>%s</Name>
			<Synonym/>
			<Comment/>
		</Properties>
		<ChildObjects/>
	</Report>
</MetaDataObject>''' % (REPORT, REPORT, REPORT)

MODULE = '&НаКлиенте\nПроцедура Выполнить(Команда)\n\tЗакрыть();\nКонецПроцедуры\n'


def main():
    work = os.path.join(HERE, '.fixture-work-v85'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log')
    src = os.path.join(work, 'src'); shutil.copytree(os.path.join(SRC, EXT), src)
    os.makedirs(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form'))
    open(os.path.join(src, 'CommonForms', FORM + '.xml'), 'w', encoding='utf-8').write(FORM_MD)
    open(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form.xml'), 'w', encoding='utf-8').write(FORM_XML)
    open(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form', 'Module.bsl'), 'w', encoding='utf-8-sig').write(MODULE)
    os.makedirs(os.path.join(src, 'Reports'))
    open(os.path.join(src, 'Reports', REPORT + '.xml'), 'w', encoding='utf-8').write(REPORT_MD)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<CommonModule>ТестРасширение_Модуль</CommonModule>',
                  '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t<CommonForm>%s</CommonForm>\n\t\t\t<Report>%s</Report>' % (FORM, REPORT))
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'v85_form')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    cfe = os.path.join(dst, 'input.cfe')
    ib = os.path.join(work, 'ib85')
    v8dump._run(EXE85, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', EXT], log)
    v8dump._run(EXE85, ['DESIGNER', '/F', ib, '/DumpCfg', cfe, '-Extension', EXT], log)
    ib = os.path.join(work, 'ib27')
    v8dump._run(EXE27, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE27, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
    v8dump._run(EXE27, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE27, ['DESIGNER', '/F', ib, '/LoadCfg', cfe, '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE27, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    shutil.copy(os.path.join(dump, 'CommonForms', FORM, 'Ext', 'Form.xml'), os.path.join(dst, 'Form.xml'))
    shutil.copy(os.path.join(dump, 'Reports', REPORT + '.xml'), os.path.join(dst, 'Report.xml'))
    body = open(cfe, 'rb').read()
    print('ok', 'Form.xml', os.path.getsize(os.path.join(dst, 'Form.xml')), 'bytes')
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
