"""Build the date choice-list fixture with the platform: the clean-room base
configuration with one common form whose date input field lists dates to
choose from (the empty date and two others), beside the platform's own dump
of the form.

    python make_choice_list_dates_fixture.py

Output: tests/fixtures/external/choice_list_dates/{input.cf, Form.xml}
and the stored choice list on stdout.
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import v8c
import make_compat_form_fixtures as compat

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
FORM = 'ФормаДаты'
FORM_UUID = '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21'
DATES = ['0001-01-01T00:00:00', '2024-01-01T00:00:00', '2024-05-17T13:45:10']

ITEM = '''					<xr:Item>
						<xr:Presentation/>
						<xr:CheckState>0</xr:CheckState>
						<xr:Value xsi:type="FormChoiceListDesTimeValue">
							<Presentation/>
							<Value xsi:type="xs:dateTime">%s</Value>
						</xr:Value>
					</xr:Item>
'''


FORM_XML = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>
	<ChildItems>
		<InputField name="Дата" id="1">
			<DataPath>Дата</DataPath>
			<ChoiceList>
%s			</ChoiceList>
			<ContextMenu name="ДатаКонтекстноеМеню" id="2"/>
			<ExtendedTooltip name="ДатаРасширеннаяПодсказка" id="3"/>
		</InputField>
	</ChildItems>
	<Attributes>
		<Attribute name="Дата" id="1">
			<Type>
				<v8:Type>xs:dateTime</v8:Type>
				<v8:DateQualifiers>
					<v8:DateFractions>DateTime</v8:DateFractions>
				</v8:DateQualifiers>
			</Type>
		</Attribute>
	</Attributes>
</Form>'''


def main():
    work = os.path.join(HERE, '.fixture-work-choice-dates'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    src = os.path.join(work, 'src'); shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
    items = ''.join(ITEM % value for value in DATES)
    os.makedirs(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form'))
    open(os.path.join(src, 'CommonForms', FORM + '.xml'), 'w', encoding='utf-8').write(compat.FORM_MD % (FORM_UUID, FORM))
    open(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form.xml'), 'w', encoding='utf-8').write(FORM_XML % items)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    anchor = '<Language>Русский</Language>'
    open(cfg, 'w', encoding='utf-8-sig').write(t.replace(anchor, anchor + '\n\t\t\t<CommonForm>%s</CommonForm>' % FORM, 1))
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'choice_list_dates')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    cf = os.path.join(dst, 'input.cf')
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', cf], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    shutil.copy(os.path.join(dump, 'CommonForms', FORM, 'Ext', 'Form.xml'), dst)
    body = v8c.read(cf)[FORM_UUID + '.0'].decode('utf-8-sig')
    for m in re.finditer(r'\{"#",0e704aa2[^\n]*', body):
        print(m.group(0)[:300])
    i = body.find('0e704aa2')
    print(body[max(0, i - 200):i + 900])
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
