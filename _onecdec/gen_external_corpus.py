"""Grow the .epf/.erf corpus from a native configuration dump.

    python gen_external_corpus.py <platform-version> <config.cf> <native-config-dump> <corpus-root> [--limit N] [--only NAME]

Every DataProcessors/<X> and Reports/<X> of the configuration dump becomes an
external data processor / report source tree (root element renamed, the
properties an external object does not have dropped, commands and the manager
module dropped, InternalInfo dropped so the platform generates it, references
to the object renamed). The platform then builds it into <corpus>/ext-bin/<X>.epf|.erf
inside an infobase that holds the configuration (so configuration types
resolve) and dumps the built file natively twice:
  <corpus>/native-<ver>/ext/<X>      from an EMPTY infobase (standalone reference)
  <corpus>/native-<ver>/ext-cfg/<X>  from the configuration infobase
Objects the platform refuses are listed in <corpus>/ext-rejected-<ver>.txt.
"""
import os, re, sys, shutil, time, uuid
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
import v8dump

DROP_PROPS = {
    'DataProcessor': ['UseStandardCommands', 'IncludeHelpInContents', 'ExtendedPresentation', 'Explanation'],
    'Report': ['UseStandardCommands', 'IncludeHelpInContents', 'ExtendedPresentation', 'Explanation'],
}
EXT = {'DataProcessor': ('ExternalDataProcessor', '.epf'), 'Report': ('ExternalReport', '.erf')}
CLASS = {'DataProcessor': 'c3831ec8-d8d5-4f93-8a22-f9bfae07327f', 'Report': 'e41aff26-25cf-4bb6-b6c1-3f478a75f374'}


def exe_for(ver):
    return r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver


def _drop_element(xml, indent, tag):
    """remove <tag/> or <tag>...</tag> written at exactly `indent`"""
    xml = re.sub(r'\r?\n%s<%s/>' % (indent, tag), '', xml)
    return re.sub(r'\r?\n%s<%s>.*?</%s>' % (indent, tag, tag), '', xml, flags=re.S)


def _rename_refs(text, kind, name):
    ext = EXT[kind][0]
    # DataProcessorObject.X / ReportTabularSectionRow.X.Y / DataProcessor.X.Form.Y ...
    pat = r'(?<![\w.])%s(Object|TabularSection|TabularSectionRow)?\.%s(?![\w])' % (kind, re.escape(name))
    return re.sub(pat, lambda m: '%s%s.%s' % (ext, m.group(1) or '', name), text)


def make_source(dump, kind, name, dst):
    folder = {'DataProcessor': 'DataProcessors', 'Report': 'Reports'}[kind]
    ext = EXT[kind][0]
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
    root = open(os.path.join(dump, folder, name + '.xml'), encoding='utf-8-sig').read()
    root = root.replace('<%s uuid=' % kind, '<%s uuid=' % ext, 1).replace('</%s>' % kind, '</%s>' % ext)
    # root InternalInfo: an external object is a contained object and has no manager type
    root = re.sub(r'\r?\n\t\t\t<xr:GeneratedType name="%sManager\.[^"]*" category="Manager">.*?</xr:GeneratedType>' % kind,
                  '', root, count=1, flags=re.S)
    # fresh type ids: the configuration infobase already owns the original ones
    root = re.sub(r'<xr:(TypeId|ValueId)>([0-9a-f-]{36})</xr:\1>',
                  lambda m: '<xr:%s>%s</xr:%s>' % (m.group(1), uuid.uuid5(uuid.NAMESPACE_URL, 'ext/' + m.group(2)), m.group(1)),
                  root)
    oid = str(uuid.uuid5(uuid.NAMESPACE_URL, 'onecdec-corpus/%s/%s' % (kind, name)))
    root = root.replace('\t\t<InternalInfo>\r\n',
                        '\t\t<InternalInfo>\r\n\t\t\t<xr:ContainedObject>\r\n\t\t\t\t<xr:ClassId>%s</xr:ClassId>\r\n'
                        '\t\t\t\t<xr:ObjectId>%s</xr:ObjectId>\r\n\t\t\t</xr:ContainedObject>\r\n' % (CLASS[kind], oid), 1)
    for p in DROP_PROPS[kind]:
        root = _drop_element(root, '\t\t\t', p)
    root = re.sub(r'\r?\n\t\t\t<Command uuid="[^"]*">.*?\r?\n\t\t\t</Command>', '', root, flags=re.S)
    root = re.sub(r'<ChildObjects>\s*</ChildObjects>', '<ChildObjects/>', root)
    root = _rename_refs(root, kind, name)
    open(os.path.join(dst, name + '.xml'), 'w', encoding='utf-8-sig', newline='').write(root)
    src_dir = os.path.join(dump, folder, name)
    if os.path.isdir(src_dir):
        shutil.copytree(src_dir, os.path.join(dst, name),
                        ignore=lambda d, fs: [f for f in fs if f == 'Commands' or f == 'ManagerModule.bsl'])
        for r, _, fs in os.walk(os.path.join(dst, name)):
            for f in fs:
                if f.endswith('.xml'):
                    p = os.path.join(r, f)
                    t = open(p, encoding='utf-8-sig').read()
                    t2 = _rename_refs(t, kind, name)
                    if t2 != t: open(p, 'w', encoding='utf-8-sig', newline='').write(t2)
    return os.path.join(dst, name + '.xml')


def main():
    a = sys.argv[1:]
    ver, cf, dump, corpus = a[:4]
    limit = int(a[a.index('--limit') + 1]) if '--limit' in a else None
    only = a[a.index('--only') + 1] if '--only' in a else None
    exe = exe_for(ver)
    work = os.path.join(corpus, 'work-ext-' + ver); os.makedirs(work, exist_ok=True)
    log = os.path.join(work, 'platform.log')
    cfg_ib, empty_ib = os.path.join(work, 'ib-cfg'), os.path.join(work, 'ib-empty')
    if not os.path.exists(os.path.join(cfg_ib, '1Cv8.1CD')):
        v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % cfg_ib], log)
        v8dump._run(exe, ['DESIGNER', '/F', cfg_ib, '/LoadCfg', os.path.abspath(cf)], log)
        v8dump._run(exe, ['DESIGNER', '/F', cfg_ib, '/UpdateDBCfg'], log)
    if not os.path.exists(os.path.join(empty_ib, '1Cv8.1CD')):
        v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % empty_ib], log)
    items = []
    for kind, folder in (('DataProcessor', 'DataProcessors'), ('Report', 'Reports')):
        for x in sorted(os.listdir(os.path.join(dump, folder))):
            if x.endswith('.xml'): items.append((kind, x[:-4]))
    if only: items = [i for i in items if i[1] == only]
    if limit: items = items[:limit]
    rejected = []
    for kind, name in items:
        ext = EXT[kind][1]
        t = time.time()
        src = make_source(dump, kind, name, os.path.join(corpus, 'ext-src', name))
        binp = os.path.join(corpus, 'ext-bin', name + ext); os.makedirs(os.path.dirname(binp), exist_ok=True)
        try:
            if os.path.exists(binp): os.remove(binp)
            v8dump._run(exe, ['DESIGNER', '/F', cfg_ib, '/LoadExternalDataProcessorOrReportFromFiles', src, binp], log)
            for sub, ib in (('ext', empty_ib), ('ext-cfg', cfg_ib)):
                out = os.path.join(corpus, 'native-' + ver, sub, name)
                shutil.rmtree(out, ignore_errors=True); os.makedirs(out)
                v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                                  os.path.join(out, name + '.xml'), binp], log)
                open(os.path.join(out, '.complete'), 'w').close()
            print('ok     %-60s %s %5.0fs' % (name, ext, time.time() - t))
        except v8dump.DumpError as ex:
            rejected.append('%s\t%s\t%s' % (name, ext, str(ex).replace('\n', ' ')[:500]))
            print('REJECT %-60s %s' % (name, str(ex).replace('\n', ' ')[:200]))
        sys.stdout.flush()
    open(os.path.join(corpus, 'ext-rejected-%s.txt' % ver), 'w', encoding='utf-8').write('\n'.join(rejected))
    print('built %d of %d' % (len(items) - len(rejected), len(items)))


if __name__ == '__main__':
    main()
