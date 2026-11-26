"""Build one-common-form configurations with the platform under several
compatibility modes and keep, per mode, the built file beside the platform's
own dump of the form -- a probe for form properties whose spelling depends on
the compatibility mode, and the fixture builder for the tests that pin them.

    python make_form_compat_probe.py <Form.xml> <out dir> <mode>... [--extension]

<mode> is a CompatibilityMode spelling (`Version8_3_17`). Output:
<out dir>/<mode>/{input.cf, Form.xml}; with --extension the form is a common
form of an extension whose ConfigurationExtensionCompatibilityMode is <mode>
(over the empty base, itself at 8.3.27): <out dir>/<mode>/{input.cfe, Form.xml}. The work directory is a fresh temp
directory of this process (other agents may run the platform concurrently).
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_compat_form_fixtures as compat

EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
FORM = 'Форма'


def build(form_xml, dst_root, mode, work):
    case_work = os.path.join(work, mode); os.makedirs(case_work)
    log = os.path.join(case_work, 'platform.log'); ib = os.path.join(case_work, 'ib')
    src = os.path.join(case_work, 'src')
    shutil.copytree(os.path.join(compat.SRC, 'cfe_base'), src)
    compat.add_form(src, FORM, '<Language>Русский</Language>',
                    '<Language>Русский</Language>\n\t\t\t<CommonForm>%s</CommonForm>')
    open(os.path.join(src, 'CommonForms', FORM, 'Ext', 'Form.xml'), 'w', encoding='utf-8-sig').write(form_xml)
    compat.set_mode(src, 'CompatibilityMode', mode)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
    dst = os.path.join(dst_root, mode); shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cf')], log)
    dump = os.path.join(case_work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
    shutil.copy(os.path.join(dump, 'CommonForms', FORM, 'Ext', 'Form.xml'), os.path.join(dst, 'Form.xml'))
    print('ok', mode, flush=True)


def build_extension(form_xml, dst_root, mode, work):
    case_work = os.path.join(work, 'cfe-' + mode); os.makedirs(case_work)
    log = os.path.join(case_work, 'platform.log'); ib = os.path.join(case_work, 'ib')
    src = os.path.join(case_work, 'src')
    form = 'ТестРасширение_Форма'
    shutil.copytree(os.path.join(compat.SRC, compat.EXT), src)
    compat.add_form(src, form, '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                    '<CommonModule>ТестРасширение_Модуль</CommonModule>\n\t\t\t<CommonForm>%s</CommonForm>')
    open(os.path.join(src, 'CommonForms', form, 'Ext', 'Form.xml'), 'w', encoding='utf-8-sig').write(form_xml)
    compat.set_mode(src, 'ConfigurationExtensionCompatibilityMode', mode)
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', os.path.join(compat.SRC, 'cfe_base')], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src, '-Extension', compat.EXT], log)
    dst = os.path.join(dst_root, mode); shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', compat.EXT], log)
    dump = os.path.join(case_work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', compat.EXT], log)
    shutil.copy(os.path.join(dump, 'CommonForms', form, 'Ext', 'Form.xml'), os.path.join(dst, 'Form.xml'))
    print('ok extension', mode, flush=True)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    form_xml = open(args[0], encoding='utf-8-sig').read()
    dst_root = args[1]
    work = tempfile.mkdtemp(prefix='ibcmd-form-compat-')
    try:
        for mode in args[2:]:
            if '--extension' in sys.argv:
                build_extension(form_xml, dst_root, mode, work)
            else:
                build(form_xml, dst_root, mode, work)
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
