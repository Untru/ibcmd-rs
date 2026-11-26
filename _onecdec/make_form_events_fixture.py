"""Build the form-events fixture with the platform: an extension (compatibility
8.3.27) with one common form whose table binds the two table events 8.3.27
has no name for (8.5 names them OnHover and OnSelectedRowsSetChange), beside
the platform's own dump of the form.

    python make_form_events_fixture.py [8.3.27.2214]

Output: tests/fixtures/external/form_events/{input.cfe, Form.xml}
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
EXT = 'ТестРасширение'
FORM = 'ТестРасширение_ФормаСобытий'
ON_HOVER = 'c676f87f-6c33-4dba-aad8-0526726d1bcf'
ON_SELECTED_ROWS_SET_CHANGE = '147fd867-8f22-4463-939d-4b48c5860c89'

FORM_MD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<CommonForm uuid="3f0c2a17-5d4e-4b8a-9c61-7e2f4a9b0d11">
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
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>
	<ChildItems>
		<Table name="Строки" id="1">
			<DataPath>Строки</DataPath>
			<ContextMenu name="СтрокиКонтекстноеМеню" id="2"/>
			<AutoCommandBar name="СтрокиКоманднаяПанель" id="3"/>
			<ExtendedTooltip name="СтрокиРасширеннаяПодсказка" id="4"/>
			<SearchStringAddition name="СтрокиСтрокаПоиска" id="5">
				<AdditionSource>
					<Item>Строки</Item>
					<Type>SearchStringRepresentation</Type>
				</AdditionSource>
				<ContextMenu name="СтрокиСтрокаПоискаКонтекстноеМеню" id="6"/>
				<ExtendedTooltip name="СтрокиСтрокаПоискаРасширеннаяПодсказка" id="7"/>
			</SearchStringAddition>
			<ViewStatusAddition name="СтрокиСостояниеПросмотра" id="8">
				<AdditionSource>
					<Item>Строки</Item>
					<Type>ViewStatusRepresentation</Type>
				</AdditionSource>
				<ContextMenu name="СтрокиСостояниеПросмотраКонтекстноеМеню" id="9"/>
				<ExtendedTooltip name="СтрокиСостояниеПросмотраРасширеннаяПодсказка" id="10"/>
			</ViewStatusAddition>
			<SearchControlAddition name="СтрокиУправлениеПоиском" id="11">
				<AdditionSource>
					<Item>Строки</Item>
					<Type>SearchControl</Type>
				</AdditionSource>
				<ContextMenu name="СтрокиУправлениеПоискомКонтекстноеМеню" id="12"/>
				<ExtendedTooltip name="СтрокиУправлениеПоискомРасширеннаяПодсказка" id="13"/>
			</SearchControlAddition>
			<Events>
				<Event name="%s">СтрокиПриНаведении</Event>
				<Event name="%s">СтрокиПриИзмененииНабораВыделенныхСтрок</Event>
			</Events>
			<ChildItems>
				<InputField name="СтрокиЗначение" id="14">
					<DataPath>Строки.Значение</DataPath>
					<ContextMenu name="СтрокиЗначениеКонтекстноеМеню" id="15"/>
					<ExtendedTooltip name="СтрокиЗначениеРасширеннаяПодсказка" id="16"/>
				</InputField>
			</ChildItems>
		</Table>
	</ChildItems>
	<Attributes>
		<Attribute name="Строки" id="1">
			<Type>
				<v8:Type>v8:ValueTable</v8:Type>
			</Type>
			<Columns>
				<Column name="Значение" id="1">
					<Type>
						<v8:Type>xs:string</v8:Type>
						<v8:StringQualifiers>
							<v8:Length>10</v8:Length>
							<v8:AllowedLength>Variable</v8:AllowedLength>
						</v8:StringQualifiers>
					</Type>
				</Column>
			</Columns>
		</Attribute>
		<Attribute name="ЛюбаяСсылка" id="2">
			<Type>
				<v8:TypeSet>cfg:AnyIBRef</v8:TypeSet>
			</Type>
		</Attribute>
	</Attributes>
</Form>''' % (ON_HOVER, ON_SELECTED_ROWS_SET_CHANGE)

MODULE = ('&НаКлиенте\nПроцедура СтрокиПриНаведении(Элемент)\nКонецПроцедуры\n\n'
          '&НаКлиенте\nПроцедура СтрокиПриИзмененииНабораВыделенныхСтрок(Элемент)\nКонецПроцедуры\n')


def main():
    # [mode]: the extension's compatibility mode; below 8.3.23 the platform
    # writes the AnyIBRef type set as cfg:AnyRef (fixture form_events_8_3_22).
    mode = sys.argv[1] if len(sys.argv) > 1 else None
    exe = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
    work = os.path.join(HERE, '.fixture-work-events'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    src = os.path.join(work, 'src'); shutil.copytree(os.path.join(SRC, EXT), src)
    os.makedirs(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form'))
    open(os.path.join(src, 'CommonForms', FORM + '.xml'), 'w', encoding='utf-8').write(FORM_MD)
    open(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form.xml'), 'w', encoding='utf-8').write(FORM_XML)
    open(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form', 'Module.bsl'), 'w', encoding='utf-8-sig').write(MODULE)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<CommonModule>ТестРасширение_Модуль</CommonModule>',
                  '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t<CommonForm>%s</CommonForm>' % FORM)
    if mode:
        t = re.sub(r'<ConfigurationExtensionCompatibilityMode>[^<]*<', '<ConfigurationExtensionCompatibilityMode>%s<' % mode, t)
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', EXT], log)
    name = 'form_events' + ('_' + mode.replace('Version', '') if mode else '')
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', name)
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    shutil.copy(os.path.join(dump, 'CommonForms', FORM, 'Ext', 'Form.xml'), os.path.join(dst, 'Form.xml'))
    print(open(os.path.join(dst, 'Form.xml'), encoding='utf-8-sig').read().count(ON_HOVER), 'uuid spellings kept')
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
