"""Case d of the S1-G proof: a new document with a tabular section, cloned from a small БСП document.

Writes F:\\ibcmd\\lab\\05\\s1g\\case_d\\stage\\{Configuration.xml, Documents\\ДемоНовыйДокумент.xml} and files.txt.
The clone drops the forms, the register records and the reference-typed attributes (only primitive types are left).
"""
import os
import re
import sys
import uuid

sys.stdout.reconfigure(encoding='utf-8')
SRC = r'F:\ibcmd\lab\04\restructure\export\s2_b1_nat'
OUT = r'F:\ibcmd\lab\05\s1g\case_d\stage'
OLD = '_ДемоОприходованиеТоваров'
NEW = 'ДемоНовыйДокумент'

raw = open(os.path.join(SRC, 'Documents', OLD + '.xml'), 'rb').read()
bom = raw.startswith(b'\xef\xbb\xbf')
text = raw.decode('utf-8-sig')

# new uuids for everything
mapping = {}


def fresh(m):
    u = m.group(0)
    if u not in mapping:
        mapping[u] = str(uuid.uuid4())
    return mapping[u]


CRLF = chr(13) + chr(10) in text
text = text.replace(chr(13) + chr(10), chr(10))
text = re.sub(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', fresh, text)
text = text.replace(OLD, NEW)
text = text.replace('Демо: Оприходование товаров', 'Демо: Новый документ')
text = text.replace('Демо: Оприходования товаров', 'Демо: Новые документы')
# an object presentation, so that facbfffe section 3 gets an entry of the document
text = text.replace('<ObjectPresentation/>',
                    '<ObjectPresentation>\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>ru</v8:lang>\n\t\t\t\t\t<v8:content>Демо: Новый документ (представление)</v8:content>\n\t\t\t\t</v8:item>\n\t\t\t</ObjectPresentation>', 1)
# no forms, no register records
text = re.sub(r'<DefaultObjectForm>[^<]*</DefaultObjectForm>', '<DefaultObjectForm/>', text)
text = re.sub(r'<DefaultListForm>[^<]*</DefaultListForm>', '<DefaultListForm/>', text)
text = re.sub(r'\t\t\t<Form>[^<]*</Form>\n', '', text)
text = re.sub(r'<RegisterRecords>.*?</RegisterRecords>', '<RegisterRecords/>', text, flags=re.S)
# only primitive-typed attributes
removed = []
for name in ('МестоХранения', 'Организация', 'Ответственный', 'Номенклатура'):
    pattern = re.compile(r'[ \t]*<Attribute uuid="[^"]+">\s*<Properties>\s*<Name>' + name + r'</Name>.*?</Attribute>\n', re.S)
    text, n = pattern.subn('', text, count=1)
    removed.append((name, n))
assert all(n == 1 for _, n in removed), removed
assert 'cfg:CatalogRef' not in text, 'a reference-typed attribute is left'

if CRLF:
    text = text.replace(chr(10), chr(13) + chr(10))
os.makedirs(os.path.join(OUT, 'Documents'), exist_ok=True)
open(os.path.join(OUT, 'Documents', NEW + '.xml'), 'wb').write((b'\xef\xbb\xbf' if bom else b'') + text.encode('utf-8'))

# Configuration.xml: the document before the first later name
craw = open(os.path.join(SRC, 'Configuration.xml'), 'rb').read()
cbom = craw.startswith(b'\xef\xbb\xbf')
ctext = craw.decode('utf-8-sig')
docs = re.findall(r'<Document>([^<]+)</Document>', ctext)
later = [n for n in docs if n > NEW]
nl = '\r\n' if '\r\n' in ctext else '\n'
if later:
    key = '\t\t\t<Document>%s</Document>' % later[0]
    ctext = ctext.replace(key, '\t\t\t<Document>%s</Document>' % NEW + nl + key, 1)
else:
    key = '\t\t\t<Document>%s</Document>' % docs[-1]
    ctext = ctext.replace(key, key + nl + '\t\t\t<Document>%s</Document>' % NEW, 1)
open(os.path.join(OUT, 'Configuration.xml'), 'wb').write((b'\xef\xbb\xbf' if cbom else b'') + ctext.encode('utf-8'))
open(os.path.join(os.path.dirname(OUT), 'files.txt'), 'w', encoding='utf-8').write('Configuration.xml\nDocuments/%s.xml\n' % NEW)
doc_uuid = re.search(r'<Document uuid="([^"]+)"', text).group(1)
print('document', NEW, doc_uuid, 'inserted before', later[:1], 'attributes removed', removed)
print(re.findall(r'<Name>([^<]+)</Name>', text[text.index('<ChildObjects>'):]))
