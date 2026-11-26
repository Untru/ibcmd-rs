"""Build a one-common-template configuration with the platform and keep the
built file beside the platform's own dump of the template -- a probe for
spreadsheet members, and the fixture builder for the tests that pin them.

    python make_template_probe.py <Template.xml> <out dir> [<CompatibilityMode>]

Output: <out dir>/{input.cf, Template.xml}. The work directory is a fresh
temp directory of this process (other agents may run the platform
concurrently).
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_compat_form_fixtures as compat

EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
NAME = 'Макет'
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


def main():
    template, dst = sys.argv[1], sys.argv[2]
    mode = sys.argv[3] if len(sys.argv) > 3 else None
    work = tempfile.mkdtemp(prefix='ibcmd-template-probe-')
    try:
        log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
        src = os.path.join(work, 'src')
        shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
        os.makedirs(os.path.join(src, 'CommonTemplates', NAME, 'Ext'))
        open(os.path.join(src, 'CommonTemplates', NAME + '.xml'), 'w', encoding='utf-8').write(MD)
        shutil.copy(template, os.path.join(src, 'CommonTemplates', NAME, 'Ext', 'Template.xml'))
        cfg = os.path.join(src, 'Configuration.xml')
        t = open(cfg, encoding='utf-8-sig').read()
        t = t.replace('<Language>Русский</Language>', '<Language>Русский</Language>\n\t\t\t<CommonTemplate>%s</CommonTemplate>' % NAME, 1)
        open(cfg, 'w', encoding='utf-8-sig').write(t)
        if mode:
            compat.set_mode(src, 'CompatibilityMode', mode)
        v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cf')], log)
        dump = os.path.join(work, 'dump')
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
        shutil.copy(os.path.join(dump, 'CommonTemplates', NAME, 'Ext', 'Template.xml'), os.path.join(dst, 'Template.xml'))
        print('ok', dst)
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
