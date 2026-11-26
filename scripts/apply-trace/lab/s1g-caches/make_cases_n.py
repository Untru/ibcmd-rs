"""The native cases N1-N5 of S1-F (docs/apply/new-object.md 5): stages of new catalogs and documents.

Reads the native export tree of the base (a restore of ddl_bsp8327_c2_native_after.bak, i.e. a database that already
had a native apply) and writes, per case, F:\\ibcmd\\lab\\05\\s1g\\n\\<case>\\stage\\{Configuration.xml, the object files}
and files.txt, for `stage_case.ps1`:

  n1  a flat catalog: string, number, date, boolean, a reference, an indexed string, one attribute with an additional order
  n2  a hierarchical catalog of folders and items: attributes ForItem / ForFolder / ForFolderAndItem, an indexed one,
      and a tabular section with a reference and an indexed attribute
  n3  a flat catalog that the configuration lists BEFORE _ДемоСтавкиНДС, and a new attribute of _ДемоСтавкиНДС
  n4  a document with a periodic numeric number, an indexed attribute and one with an additional order
  n5  a catalog and a document together
"""
import os
import re
import shutil
import sys
import uuid

sys.stdout.reconfigure(encoding='utf-8')
TREE = r'F:\ibcmd\lab\05\s1g\n\tree'
OUT = r'F:\ibcmd\lab\05\s1g\n'
UUID = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}')
CR, LF = chr(13), chr(10)


def read(path):
    raw = open(path, 'rb').read()
    bom = raw.startswith(b'\xef\xbb\xbf')
    text = raw.decode('utf-8-sig')
    crlf = CR + LF in text
    return text.replace(CR + LF, LF), bom, crlf


def write(path, text, bom, crlf):
    if crlf:
        text = text.replace(LF, CR + LF)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, 'wb').write((b'\xef\xbb\xbf' if bom else b'') + text.encode('utf-8'))


def renew(text):
    """Every uuid of the text becomes a new one (the same old uuid the same new one)."""
    mapping = {}

    def fresh(m):
        return mapping.setdefault(m.group(0), str(uuid.uuid4()))

    return UUID.sub(fresh, text)


def synonym(text):
    return ('<Synonym>\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>ru</v8:lang>\n\t\t\t\t\t<v8:content>%s</v8:content>\n'
            '\t\t\t\t</v8:item>\n\t\t\t</Synonym>') % text


TYPES = {
    'string': lambda n: ['<v8:Type>xs:string</v8:Type>', '<v8:StringQualifiers>', '\t<v8:Length>%d</v8:Length>' % n,
                         '\t<v8:AllowedLength>Variable</v8:AllowedLength>', '</v8:StringQualifiers>'],
    'number': lambda d, f: ['<v8:Type>xs:decimal</v8:Type>', '<v8:NumberQualifiers>', '\t<v8:Digits>%d</v8:Digits>' % d,
                            '\t<v8:FractionDigits>%d</v8:FractionDigits>' % f, '\t<v8:AllowedSign>Any</v8:AllowedSign>',
                            '</v8:NumberQualifiers>'],
    'date': lambda: ['<v8:Type>xs:dateTime</v8:Type>', '<v8:DateQualifiers>', '\t<v8:DateFractions>Date</v8:DateFractions>',
                     '</v8:DateQualifiers>'],
    'boolean': lambda: ['<v8:Type>xs:boolean</v8:Type>'],
    'ref': lambda name: ['<v8:Type>cfg:%s</v8:Type>' % name],
}


def attribute(name, syn, type_xml, indent, use=None, indexing='DontIndex'):
    """An <Attribute> block (export layout 2.20). `use` for a catalog attribute: ForItem, ForFolder, ForFolderAndItem."""
    pad = '\t' * indent
    lines = [
        '<Attribute uuid="%s">' % uuid.uuid4(),
        '\t<Properties>',
        '\t\t<Name>%s</Name>' % name,
        '\t\t<Synonym>', '\t\t\t<v8:item>', '\t\t\t\t<v8:lang>ru</v8:lang>',
        '\t\t\t\t<v8:content>%s</v8:content>' % syn, '\t\t\t</v8:item>', '\t\t</Synonym>',
        '\t\t<Comment/>', '\t\t<Type>',
    ] + ['\t\t\t' + x for x in type_xml] + [
        '\t\t</Type>',
        '\t\t<PasswordMode>false</PasswordMode>', '\t\t<Format/>', '\t\t<EditFormat/>', '\t\t<ToolTip/>',
        '\t\t<MarkNegatives>false</MarkNegatives>', '\t\t<Mask/>', '\t\t<MultiLine>false</MultiLine>',
        '\t\t<ExtendedEdit>false</ExtendedEdit>', '\t\t<MinValue xsi:nil="true"/>', '\t\t<MaxValue xsi:nil="true"/>',
        '\t\t<FillFromFillingValue>false</FillFromFillingValue>', '\t\t<FillValue xsi:nil="true"/>',
        '\t\t<FillChecking>DontCheck</FillChecking>', '\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>',
        '\t\t<ChoiceParameterLinks/>', '\t\t<ChoiceParameters/>', '\t\t<QuickChoice>Auto</QuickChoice>',
        '\t\t<CreateOnInput>Auto</CreateOnInput>', '\t\t<ChoiceForm/>', '\t\t<LinkByType/>',
        '\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>',
    ] + (['\t\t<Use>%s</Use>' % use] if use else []) + [
        '\t\t<Indexing>%s</Indexing>' % indexing,
        '\t\t<FullTextSearch>Use</FullTextSearch>', '\t\t<DataHistory>Use</DataHistory>',
        '\t</Properties>', '</Attribute>',
    ]
    return LF.join(pad + x for x in lines)


def table_attribute(name, syn, type_xml, indent, indexing='DontIndex'):
    """An attribute of a tabular section: no <FillFromFillingValue>/<FillValue>, no <Use>."""
    block = attribute(name, syn, type_xml, indent, indexing=indexing)
    block = re.sub(r'[ \t]*<FillFromFillingValue>false</FillFromFillingValue>\n', '', block)
    block = re.sub(r'[ \t]*<FillValue xsi:nil="true"/>\n', '', block)
    return block


def table_section(name, syn, attributes, indent):
    pad = '\t' * indent
    head = [
        '<TabularSection uuid="%s">' % uuid.uuid4(),
        '\t<InternalInfo>',
        '\t\t<xr:GeneratedType name="CatalogTabularSection.@OWNER@.%s" category="TabularSection">' % name,
        '\t\t\t<xr:TypeId>%s</xr:TypeId>' % uuid.uuid4(),
        '\t\t\t<xr:ValueId>%s</xr:ValueId>' % uuid.uuid4(),
        '\t\t</xr:GeneratedType>',
        '\t\t<xr:GeneratedType name="CatalogTabularSectionRow.@OWNER@.%s" category="TabularSectionRow">' % name,
        '\t\t\t<xr:TypeId>%s</xr:TypeId>' % uuid.uuid4(),
        '\t\t\t<xr:ValueId>%s</xr:ValueId>' % uuid.uuid4(),
        '\t\t</xr:GeneratedType>',
        '\t</InternalInfo>',
        '\t<Properties>',
        '\t\t<Name>%s</Name>' % name,
        '\t\t<Synonym>', '\t\t\t<v8:item>', '\t\t\t\t<v8:lang>ru</v8:lang>',
        '\t\t\t\t<v8:content>%s</v8:content>' % syn, '\t\t\t</v8:item>', '\t\t</Synonym>',
        '\t\t<Comment/>', '\t\t<ToolTip/>', '\t\t<FillChecking>DontCheck</FillChecking>',
        '\t\t<Use>ForItem</Use>', '\t\t<LineNumberLength>5</LineNumberLength>',
        '\t</Properties>',
        '\t<ChildObjects>',
    ]
    body = LF.join(pad + x for x in head) + LF + attributes + LF + LF.join(pad + x for x in ['\t</ChildObjects>', '</TabularSection>'])
    return body


def catalog_from(skeleton, name, syn, attributes_xml, sections_xml='', hierarchical=None, code=None, description=None):
    text, bom, crlf = read(os.path.join(TREE, 'Catalogs', skeleton + '.xml'))
    old = skeleton
    text = renew(text)
    text = text.replace(old, name)
    text = re.sub(r'<v8:content>Демо: Новый справочник</v8:content>', '<v8:content>%s</v8:content>' % syn, text, count=1)
    text = text.replace('@OWNER@', name)
    if hierarchical is not None:
        text = re.sub(r'<Hierarchical>\w+</Hierarchical>', '<Hierarchical>%s</Hierarchical>' % ('true' if hierarchical else 'false'), text, count=1)
    if code is not None:
        text = re.sub(r'<CodeLength>\d+</CodeLength>', '<CodeLength>%d</CodeLength>' % code, text, count=1)
    if description is not None:
        text = re.sub(r'<DescriptionLength>\d+</DescriptionLength>', '<DescriptionLength>%d</DescriptionLength>' % description, text, count=1)
    children = attributes_xml + (LF + sections_xml if sections_xml else '')
    assert '<ChildObjects/>' in text
    text = text.replace('\t\t<ChildObjects/>', '\t\t<ChildObjects>' + LF + children.replace('@OWNER@', name) + LF + '\t\t</ChildObjects>', 1)
    return text, bom, crlf


def document_from(skeleton, name, syn, number=None):
    text, bom, crlf = read(os.path.join(TREE, 'Documents', skeleton + '.xml'))
    text = renew(text)
    text = text.replace(skeleton, name)
    text = text.replace('Демо: Оприходование товаров', syn).replace('Демо: Оприходования товаров', syn + ' (список)')
    text = re.sub(r'<DefaultObjectForm>[^<]*</DefaultObjectForm>', '<DefaultObjectForm/>', text)
    text = re.sub(r'<DefaultListForm>[^<]*</DefaultListForm>', '<DefaultListForm/>', text)
    text = re.sub(r'\t\t\t<Form>[^<]*</Form>\n', '', text)
    text = re.sub(r'<RegisterRecords>.*?</RegisterRecords>', '<RegisterRecords/>', text, flags=re.S)
    text = text.replace('<ObjectPresentation/>', '<ObjectPresentation>\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>ru</v8:lang>\n\t\t\t\t\t<v8:content>%s</v8:content>\n\t\t\t\t</v8:item>\n\t\t\t</ObjectPresentation>' % syn, 1)
    for attr in ('МестоХранения', 'Организация', 'Ответственный', 'Номенклатура'):
        pattern = re.compile(r'[ \t]*<Attribute uuid="[^"]+">\s*<Properties>\s*<Name>' + attr + r'</Name>.*?</Attribute>\n', re.S)
        text, n = pattern.subn('', text, count=1)
        assert n == 1, attr
    if number:
        for key, value in number.items():
            text = re.sub(r'<%s>[^<]*</%s>' % (key, key), '<%s>%s</%s>' % (key, value, key), text, count=1)
    assert 'cfg:CatalogRef' not in text
    return text, bom, crlf


def add_to_document(text, extra_attributes):
    """Attributes after the last object-level attribute, before the tabular section."""
    marker = '\t\t\t<TabularSection uuid='
    at = text.index(marker)
    return text[:at] + extra_attributes + LF + text[at:]


def configuration(kind, names, before=None):
    text, bom, crlf = read(os.path.join(TREE, 'Configuration.xml'))
    existing = re.findall(r'<%s>([^<]+)</%s>' % (kind, kind), text)
    for name in names:
        if before and name in before:
            key = '\t\t\t<%s>%s</%s>' % (kind, before[name], kind)
            assert key in text, key
            text = text.replace(key, '\t\t\t<%s>%s</%s>' % (kind, name, kind) + LF + key, 1)
            continue
        later = [n for n in existing if n > name]
        if later:
            key = '\t\t\t<%s>%s</%s>' % (kind, later[0], kind)
            text = text.replace(key, '\t\t\t<%s>%s</%s>' % (kind, name, kind) + LF + key, 1)
        else:
            key = '\t\t\t<%s>%s</%s>' % (kind, existing[-1], kind)
            text = text.replace(key, key + LF + '\t\t\t<%s>%s</%s>' % (kind, name, kind), 1)
    return text, bom, crlf


ONLY = set(sys.argv[1:])   # the cases to write (the uuids are new at every run: the others keep the ones the platform has seen)


def emit(case, files, listing_edits):
    if ONLY and case not in ONLY:
        return
    stage = os.path.join(OUT, case, 'stage')
    for rel, (text, bom, crlf) in files.items():
        write(os.path.join(stage, rel), text, bom, crlf)
    text, bom, crlf = read(os.path.join(TREE, 'Configuration.xml'))
    for kind, names, before in listing_edits:
        pass
    write(os.path.join(stage, 'Configuration.xml'), *CONFIG[case])
    listing = ['Configuration.xml'] + sorted(k.replace(os.sep, '/') for k in files)
    open(os.path.join(OUT, case, 'files.txt'), 'w', encoding='utf-8').write('\n'.join(listing) + '\n')
    print(case, [k for k in files])


CONFIG = {}


def config_for(case, edits):
    text, bom, crlf = read(os.path.join(TREE, 'Configuration.xml'))
    for kind, names, before in edits:
        existing = re.findall(r'<%s>([^<]+)</%s>' % (kind, kind), text)
        for name in names:
            if before and name in before:
                key = '\t\t\t<%s>%s</%s>' % (kind, before[name], kind)
                assert key in text, key
                text = text.replace(key, '\t\t\t<%s>%s</%s>' % (kind, name, kind) + LF + key, 1)
                continue
            later = [n for n in existing if n > name]
            if later:
                key = '\t\t\t<%s>%s</%s>' % (kind, later[0], kind)
                text = text.replace(key, '\t\t\t<%s>%s</%s>' % (kind, name, kind) + LF + key, 1)
            else:
                key = '\t\t\t<%s>%s</%s>' % (kind, existing[-1], kind)
                text = text.replace(key, key + LF + '\t\t\t<%s>%s</%s>' % (kind, name, kind), 1)
    CONFIG[case] = (text, bom, crlf)


def main():
    flat = 'ДемоНовыйСправочник'   # the catalog of case c: flat, code 9, description 25, no attributes, no forms

    # n1
    a = LF.join([
        attribute('Строка50', 'Строка 50', TYPES['string'](50), 3),
        attribute('Число', 'Число', TYPES['number'](10, 2), 3),
        attribute('Дата', 'Дата', TYPES['date'](), 3),
        attribute('Флаг', 'Флаг', TYPES['boolean'](), 3),
        attribute('Пользователь', 'Пользователь', TYPES['ref']('CatalogRef.Пользователи'), 3),
        attribute('ИндексСтрока', 'Индекс строка', TYPES['string'](30), 3, indexing='Index'),
        attribute('ПорядокСтрока', 'Порядок строка', TYPES['string'](20), 3, indexing='IndexWithAdditionalOrder'),
    ])
    n1 = 'ДемоКатН1'
    config_for('n1', [('Catalog', [n1], None)])
    emit('n1', {'Catalogs/%s.xml' % n1: catalog_from(flat, n1, 'Демо: каталог Н1', a)}, [])

    # n2: hierarchical catalog of folders and items, a tabular section
    a2 = LF.join([
        attribute('ДляЭлементов', 'Для элементов', TYPES['string'](40), 3, use='ForItem'),
        attribute('ДляГрупп', 'Для групп', TYPES['number'](8, 0), 3, use='ForFolder'),
        attribute('Общий', 'Общий', TYPES['date'](), 3, use='ForFolderAndItem'),
        attribute('ИндексЭл', 'Индекс', TYPES['string'](25), 3, use='ForFolderAndItem', indexing='Index'),
    ])
    ts_attrs = LF.join([
        table_attribute('Количество', 'Количество', TYPES['number'](10, 3), 5),
        table_attribute('Сотрудник', 'Сотрудник', TYPES['ref']('CatalogRef.Пользователи'), 5),
        table_attribute('ИндексТЧ', 'Индекс ТЧ', TYPES['string'](12), 5, indexing='Index'),
    ])
    n2 = 'ДемоКатН2'
    ts = table_section('Строки', 'Строки', ts_attrs, 3)
    config_for('n2', [('Catalog', [n2], None)])
    emit('n2', {'Catalogs/%s.xml' % n2: catalog_from(flat, n2, 'Демо: каталог Н2', a2, ts, hierarchical=True)}, [])

    # n3: a new catalog listed before _ДемоСтавкиНДС and a new attribute of _ДемоСтавкиНДС (the natural choices are refused, and a refusal test each:
    # the extension adopts _ДемоПартнеры; _ДемоКассы is subordinate to owners)
    n3 = 'ДемоКатН3'
    a3 = LF.join([attribute('Наименование3', 'Наименование 3', TYPES['string'](35), 3)])
    partners, pbom, pcrlf = read(os.path.join(TREE, 'Catalogs', '_ДемоСтавкиНДС.xml'))
    extra3 = attribute('ДемоН3Реквизит', 'Демо Н3 реквизит', TYPES['string'](45), 3)
    end = '\t\t\t</Attribute>'   # _ДемоСтавкиНДС: flat, no owners, one attribute, no tabular sections
    at = partners.rindex(end) + len(end)
    partners = partners[:at] + LF + extra3 + partners[at:]
    config_for('n3', [('Catalog', [n3], {n3: '_ДемоСтавкиНДС'})])
    emit('n3', {'Catalogs/%s.xml' % n3: catalog_from(flat, n3, 'Демо: каталог Н3', a3),
                'Catalogs/_ДемоСтавкиНДС.xml': (partners, pbom, pcrlf)}, [])
    # the partial import checks that the forms the changed catalog names exist (it stages only the listed files)
    if (not ONLY or 'n3' in ONLY) and os.path.isdir(os.path.join(TREE, 'Catalogs', '_ДемоСтавкиНДС')):
        shutil.copytree(os.path.join(TREE, 'Catalogs', '_ДемоСтавкиНДС'),
                        os.path.join(OUT, 'n3', 'stage', 'Catalogs', '_ДемоСтавкиНДС'), dirs_exist_ok=True)

    # n4: a document with a periodic numeric number
    d4 = 'ДемоДокН4'
    text, bom, crlf = document_from('_ДемоОприходованиеТоваров', d4, 'Демо: документ Н4',
                                    number={'NumberType': 'Number', 'NumberLength': '12', 'NumberPeriodicity': 'Year'})
    extra = LF.join([
        attribute('ИндексЧисло', 'Индекс число', TYPES['number'](9, 0), 3, indexing='Index'),
        attribute('ПорядокСтрока', 'Порядок строка', TYPES['string'](20), 3, indexing='IndexWithAdditionalOrder'),
    ])
    text = add_to_document(text, extra)
    config_for('n4', [('Document', [d4], None)])
    emit('n4', {'Documents/%s.xml' % d4: (text, bom, crlf)}, [])

    # n5: a catalog and a document together
    c5, d5 = 'ДемоКатН5', 'ДемоДокН5'
    a5 = LF.join([attribute('Описание5', 'Описание 5', TYPES['string'](60), 3)])
    dtext, dbom, dcrlf = document_from('_ДемоОприходованиеТоваров', d5, 'Демо: документ Н5')
    config_for('n5', [('Catalog', [c5], None), ('Document', [d5], None)])
    emit('n5', {'Catalogs/%s.xml' % c5: catalog_from(flat, c5, 'Демо: каталог Н5', a5),
                'Documents/%s.xml' % d5: (dtext, dbom, dcrlf)}, [])

    # n6: like n3, but the new catalog is listed AFTER the changed one (the alphabetical place): does the platform number the
    # created objects first, or in the configuration's order?
    n6 = 'ДемоКатН6'
    a6 = LF.join([attribute('Наименование6', 'Наименование 6', TYPES['string'](35), 3)])
    changed, cbom, ccrlf = read(os.path.join(TREE, 'Catalogs', '_ДемоСтавкиНДС.xml'))
    end = '\t\t\t</Attribute>'
    at = changed.rindex(end) + len(end)
    changed6 = changed[:at] + LF + attribute('ДемоН6Реквизит', 'Демо Н6 реквизит', TYPES['string'](45), 3) + changed[at:]
    config_for('n6', [('Catalog', [n6], None)])
    emit('n6', {'Catalogs/%s.xml' % n6: catalog_from(flat, n6, 'Демо: каталог Н6', a6),
                'Catalogs/_ДемоСтавкиНДС.xml': (changed6, cbom, ccrlf)}, [])
    if not ONLY or 'n6' in ONLY:
        shutil.copytree(os.path.join(TREE, 'Catalogs', '_ДемоСтавкиНДС'),
                        os.path.join(OUT, 'n6', 'stage', 'Catalogs', '_ДемоСтавкиНДС'), dirs_exist_ok=True)

    # n7: a new document and a changed catalog (a new attribute): the kinds across created and changed objects
    d7 = 'ДемоДокН7'
    dtext7, dbom7, dcrlf7 = document_from('_ДемоОприходованиеТоваров', d7, 'Демо: документ Н7')
    changed7 = changed[:at] + LF + attribute('ДемоН7Реквизит', 'Демо Н7 реквизит', TYPES['string'](45), 3) + changed[at:]
    config_for('n7', [('Document', [d7], None)])
    emit('n7', {'Documents/%s.xml' % d7: (dtext7, dbom7, dcrlf7),
                'Catalogs/_ДемоСтавкиНДС.xml': (changed7, cbom, ccrlf)}, [])
    if not ONLY or 'n7' in ONLY:
        shutil.copytree(os.path.join(TREE, 'Catalogs', '_ДемоСтавкиНДС'),
                        os.path.join(OUT, 'n7', 'stage', 'Catalogs', '_ДемоСтавкиНДС'), dirs_exist_ok=True)


main()
