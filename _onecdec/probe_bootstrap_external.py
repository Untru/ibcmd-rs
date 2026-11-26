"""PROBE (throwaway): turn an external object's XML tree into a one-object
configuration tree and run `ibcmd-rs cf bootstrap` on it, to see which parts
the stock compiler accepts.

    python probe_bootstrap_external.py <external-dump-dir> <work-dir>
"""
import os, re, shutil, subprocess, sys, uuid

src, work = sys.argv[1], sys.argv[2]
EXE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'target', 'release', 'ibcmd-rs.exe')
root_xml = [f for f in os.listdir(src) if f.endswith('.xml')][0]
name = root_xml[:-4]
text = open(os.path.join(src, root_xml), encoding='utf-8-sig').read()
kind = 'DataProcessor' if '<ExternalDataProcessor ' in text else 'Report'
ext = 'External' + kind
folder = kind + 's'
object_id = re.search(r'<xr:ObjectId>([^<]+)</xr:ObjectId>', text).group(1)

tree = os.path.join(work, 'src')
shutil.rmtree(work, ignore_errors=True); os.makedirs(os.path.join(tree, folder))


def internal(t):
    t = t.replace(ext + 'Object.' + name, kind + 'Object.' + name)
    return re.sub(r'(?<![\w.])' + ext + r'\.' + re.escape(name) + r'(?![\w])', kind + '.' + name, t)


t = text.replace('<%s uuid="' % ext, '<%s uuid="' % kind, 1).replace('</%s>' % ext, '</%s>' % kind)
t = re.sub(r'<%s uuid="[^"]+">' % kind, '<%s uuid="%s">' % (kind, object_id), t, count=1)
t = re.sub(r'\s*<xr:ContainedObject>.*?</xr:ContainedObject>', '', t, count=1, flags=re.S)
mt, mv = uuid.uuid4(), uuid.uuid4()
t = t.replace('\t\t</InternalInfo>', '\t\t\t<xr:GeneratedType name="%sManager.%s" category="Manager">\r\n\t\t\t\t<xr:TypeId>%s</xr:TypeId>\r\n\t\t\t\t<xr:ValueId>%s</xr:ValueId>\r\n\t\t\t</xr:GeneratedType>\r\n\t\t</InternalInfo>' % (kind, name, mt, mv), 1)
t = t.replace('\t\t\t<DefaultForm>', '\t\t\t<UseStandardCommands>true</UseStandardCommands>\r\n\t\t\t<DefaultForm>', 1)
t = t.replace('\t\t</Properties>', '\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\t\t\t<ExtendedPresentation/>\r\n\t\t\t<Explanation/>\r\n\t\t</Properties>', 1)
open(os.path.join(tree, folder, name + '.xml'), 'w', encoding='utf-8-sig', newline='').write(internal(t))
if os.path.isdir(os.path.join(src, name)):
    shutil.copytree(os.path.join(src, name), os.path.join(tree, folder, name))
    for dp, _, fs in os.walk(os.path.join(tree, folder, name)):
        for f in fs:
            if f.endswith('.xml') or f.endswith('.html'):
                p = os.path.join(dp, f); s = open(p, encoding='utf-8-sig').read()
                if internal(s) != s: open(p, 'w', encoding='utf-8-sig', newline='').write(internal(s))
open(os.path.join(tree, 'Configuration.xml'), 'w', encoding='utf-8').write('''<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
  <Configuration uuid="10000000-0000-4000-8000-000000000001">
    <Properties>
      <Name>ExternalHost</Name>
      <Synonym/>
      <Comment/>
      <DefaultRunMode>ManagedApplication</DefaultRunMode>
      <ScriptVariant>Russian</ScriptVariant>
      <CompatibilityMode>Version8_3_24</CompatibilityMode>
    </Properties>
    <ChildObjects>
      <%s>%s</%s>
    </ChildObjects>
  </Configuration>
</MetaDataObject>
''' % (kind, name, kind))
out = os.path.join(work, 'out.cf')
r = subprocess.run([EXE, 'cf', 'bootstrap', tree, out, '--source-version', '2.20', '--target-profile', 'platform-8.3.27.2214'],
                   capture_output=True)
print('exit', r.returncode)
print((r.stderr or r.stdout).decode('utf-8', 'replace')[:4000])
