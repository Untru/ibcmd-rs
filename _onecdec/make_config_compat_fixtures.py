"""Build configuration compatibility fixtures with the platform: the clean-room
base configuration under pairs of CompatibilityMode /
ConfigurationExtensionCompatibilityMode, beside the platform's own dump of
Configuration.xml. The tuple stores the two in fields 26 and 43; 8.3.27.2214
prints field 43 when it is not below field 26, and its own version otherwise.

    python make_config_compat_fixtures.py

Output: tests/fixtures/external/config_compat/<case>/{input.cf, Configuration.xml}
"""
import os, re, shutil, sys
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SRC = os.path.join(HERE, 'fixture_src')
EXE = r'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8.exe'
CASES = {
    'c10_e27': ('Version8_3_10', 'Version8_3_27'),   # 43 above 26: printed as stored
    'c19_e12': ('Version8_3_19', 'Version8_3_12'),   # 43 below 26: the platform's own
    'c21_e24': ('Version8_3_21', 'Version8_3_24'),   # 43 above 26, below the platform
}


def main():
    work = os.path.join(HERE, '.fixture-work-config-compat'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    dst_root = os.path.join(REPO, 'tests', 'fixtures', 'external', 'config_compat')
    shutil.rmtree(dst_root, ignore_errors=True); os.makedirs(dst_root)
    for case, (compat, ext) in CASES.items():
        w = os.path.join(work, case); os.makedirs(w)
        log = os.path.join(w, 'platform.log'); ib = os.path.join(w, 'ib')
        src = os.path.join(w, 'src'); shutil.copytree(os.path.join(SRC, 'cfe_base'), src)
        cfg = os.path.join(src, 'Configuration.xml')
        t = open(cfg, encoding='utf-8-sig').read()
        t = re.sub(r'<CompatibilityMode>[^<]*<', '<CompatibilityMode>%s<' % compat, t)
        t = re.sub(r'<ConfigurationExtensionCompatibilityMode>[^<]*<', '<ConfigurationExtensionCompatibilityMode>%s<' % ext, t)
        open(cfg, 'w', encoding='utf-8-sig').write(t)
        dst = os.path.join(dst_root, case); os.makedirs(dst)
        v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', src], log)
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cf')], log)
        dump = os.path.join(w, 'dump')
        v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump], log)
        shutil.copy(os.path.join(dump, 'Configuration.xml'), dst)
        got = open(os.path.join(dst, 'Configuration.xml'), encoding='utf-8-sig').read()
        print('ok', case, re.findall(r'<(\w*CompatibilityMode)>([^<]*)<', got))
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
