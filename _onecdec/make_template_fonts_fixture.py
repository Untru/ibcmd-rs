"""Probe: how 8.3.27.2214 spells a template's style-item fonts in an extension
of a given compatibility mode.

    python .fixture-work-fonts.py <Version8_3_N> ...
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump
SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'fixture_src')
MXL = os.path.join(SRC, 'ТестОбработка', 'ТестОбработка', 'Templates', 'Макет', 'Ext', 'Template.xml')
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
EXT = 'ТестРасширение'
NAME = 'ТестРасширение_Макет'
MD = '''\ufeff<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommonTemplate uuid="5a1d2c3b-4e5f-4061-8293-a4b5c6d7e8f9">
		<Properties>
			<Name>%s</Name>
			<Synonym/>
			<Comment/>
			<TemplateType>SpreadsheetDocument</TemplateType>
		</Properties>
	</CommonTemplate>
</MetaDataObject>''' % NAME
HERE = os.path.dirname(os.path.abspath(__file__))
DST = os.path.join(os.path.dirname(HERE), 'tests', 'fixtures', 'external', 'template_fonts')
for mode in sys.argv[1:]:
    work = os.path.join(HERE, '.fixture-work-fonts', mode)
    shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    src = os.path.join(work, 'src'); shutil.copytree(os.path.join(SRC, EXT), src)
    mxl = open(MXL, encoding='utf-8-sig').read()
    mxl = mxl.replace('<font faceName="Arial" height="10" bold="true" italic="false" underline="false" strikeout="false" kind="Absolute" scale="100"/>',
                      '<font ref="-59" kind="StyleItem"/>', 1)
    mxl = mxl.replace('<font faceName="Arial" height="10" bold="false" italic="false" underline="false" strikeout="false" kind="Absolute" scale="100"/>',
                      '<font ref="-51" kind="StyleItem"/>', 1)
    os.makedirs(os.path.join(src, 'CommonTemplates', NAME, 'Ext'))
    open(os.path.join(src, 'CommonTemplates', NAME + '.xml'), 'w', encoding='utf-8').write(MD)
    open(os.path.join(src, 'CommonTemplates', NAME, 'Ext', 'Template.xml'), 'w', encoding='utf-8-sig').write(mxl)
    cfg = os.path.join(src, 'Configuration.xml')
    t = open(cfg, encoding='utf-8-sig').read()
    t = t.replace('<CommonModule>ТестРасширение_Модуль</CommonModule>', '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t<CommonTemplate>%s</CommonTemplate>' % NAME)
    t = re.sub(r'<ConfigurationExtensionCompatibilityMode>[^<]*<', '<ConfigurationExtensionCompatibilityMode>%s<' % mode, t)
    open(cfg, 'w', encoding='utf-8-sig').write(t)
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(SRC, 'cfe_base')], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', EXT], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(work, 'probe.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    got = open(os.path.join(dump, 'CommonTemplates', NAME, 'Ext', 'Template.xml'), encoding='utf-8-sig').read()
    shutil.rmtree(DST, ignore_errors=True); os.makedirs(DST)
    shutil.copy(os.path.join(work, 'probe.cfe'), os.path.join(DST, 'input.cfe'))
    shutil.copy(os.path.join(dump, 'CommonTemplates', NAME, 'Ext', 'Template.xml'), os.path.join(DST, 'Template.xml'))
    print(mode, re.findall(r'<font ref="[^"]*"[^>]*>', got), flush=True)
    shutil.rmtree(work, ignore_errors=True)
