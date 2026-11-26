"""Tree edits of the import matrix (issue #388): one function per kind of change, each applied to a
working copy of an exported БСП 8.3.27 tree and reverted from the base.

  python edits.py list
  python edits.py apply  <name>[,<name>...] --base <tree> --work <tree>
  python edits.py reset  --base <tree> --work <tree>
  python edits.py info   <name>

The base tree is never written. `apply` records what it touched in <work>.touched.json (next to the work
tree, not in it), `reset` puts every touched file back (or deletes the files the edit created).
Trees are UTF-8 with BOM and CRLF; every edit keeps that.
"""
import argparse
import json
import os
import re
import shutil
import sys
import uuid as uuidlib

UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


class Tree:
    def __init__(self, base, work):
        self.base = base
        self.work = work
        self.touched_path = work.rstrip("\\/") + ".touched.json"
        self.touched = []
        if os.path.exists(self.touched_path):
            with open(self.touched_path, encoding="utf-8") as f:
                self.touched = json.load(f)

    def _save(self):
        with open(self.touched_path, "w", encoding="utf-8") as f:
            json.dump(self.touched, f, ensure_ascii=False, indent=1)

    def read(self, rel):
        """Text of a file as the edits so far leave it (the work tree's when an earlier edit wrote it,
        else the base's): LF line ends, no BOM."""
        path = os.path.join(self.work, rel)
        if rel not in self.touched or not os.path.exists(path):
            path = os.path.join(self.base, rel)
        with open(path, "rb") as f:
            raw = f.read()
        # Structural CRLF becomes "\n"; a bare LF inside a text value (the export writes multi-line
        # values with LF) is kept as "\x01" so that `write` puts it back untouched.
        return raw.decode("utf-8-sig").replace("\r\n", "\x02").replace("\n", "\x01").replace("\x02", "\n")

    def exists(self, rel):
        return os.path.exists(os.path.join(self.base, rel))

    def write(self, rel, text):
        path = os.path.join(self.work, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        if rel not in self.touched:
            self.touched.append(rel)
            self._save()
        with open(path, "wb") as f:
            f.write(b"\xef\xbb\xbf" + text.replace("\n", "\r\n").replace("\x01", "\n").encode("utf-8"))

    def write_raw(self, rel, data):
        path = os.path.join(self.work, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        if rel not in self.touched:
            self.touched.append(rel)
            self._save()
        with open(path, "wb") as f:
            f.write(data)

    def delete(self, rel):
        path = os.path.join(self.work, rel)
        if rel not in self.touched:
            self.touched.append(rel)
            self._save()
        if os.path.isdir(path):
            shutil.rmtree(path)
        elif os.path.exists(path):
            os.remove(path)

    def reset(self):
        for rel in self.touched:
            src = os.path.join(self.base, rel)
            dst = os.path.join(self.work, rel)
            if os.path.isdir(src):
                if os.path.exists(dst):
                    shutil.rmtree(dst)
                shutil.copytree(src, dst)
            elif os.path.exists(src):
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copyfile(src, dst)
            elif os.path.exists(dst):
                os.remove(dst)
        self.touched = []
        self._save()


def fresh_ids(text, keep=()):
    """Replace every uuid of `text` by a new one (the same old uuid maps to the same new one)."""
    mapping = {}

    def repl(m):
        u = m.group(0)
        if u in keep:
            return u
        mapping.setdefault(u, str(uuidlib.uuid4()))
        return mapping[u]

    return UUID_RE.sub(repl, text), mapping


def synonym_xml(text, tabs):
    t = "\t" * tabs
    return "\n".join([
        t + "<Synonym>",
        t + "\t<v8:item>",
        t + "\t\t<v8:lang>ru</v8:lang>",
        t + "\t\t<v8:content>%s</v8:content>" % text,
        t + "\t</v8:item>",
        t + "</Synonym>",
    ])


def string_type(n):
    return ["<v8:Type>xs:string</v8:Type>", "<v8:StringQualifiers>", "\t<v8:Length>%d</v8:Length>" % n,
            "\t<v8:AllowedLength>Variable</v8:AllowedLength>", "</v8:StringQualifiers>"]


def attribute_block(name, synonym, type_lines, tabs=3, use=True):
    """An <Attribute> of a catalog as the 2.20 export writes it."""
    t = "\t" * tabs
    lines = [
        '<Attribute uuid="%s">' % uuidlib.uuid4(),
        "\t<Properties>",
        "\t\t<Name>%s</Name>" % name,
        "\t\t<Synonym>",
        "\t\t\t<v8:item>",
        "\t\t\t\t<v8:lang>ru</v8:lang>",
        "\t\t\t\t<v8:content>%s</v8:content>" % synonym,
        "\t\t\t</v8:item>",
        "\t\t</Synonym>",
        "\t\t<Comment/>",
        "\t\t<Type>",
    ] + ["\t\t\t" + x for x in type_lines] + [
        "\t\t</Type>",
        "\t\t<PasswordMode>false</PasswordMode>",
        "\t\t<Format/>",
        "\t\t<EditFormat/>",
        "\t\t<ToolTip/>",
        "\t\t<MarkNegatives>false</MarkNegatives>",
        "\t\t<Mask/>",
        "\t\t<MultiLine>false</MultiLine>",
        "\t\t<ExtendedEdit>false</ExtendedEdit>",
        '\t\t<MinValue xsi:nil="true"/>',
        '\t\t<MaxValue xsi:nil="true"/>',
        "\t\t<FillFromFillingValue>false</FillFromFillingValue>",
        '\t\t<FillValue xsi:nil="true"/>',
        "\t\t<FillChecking>DontCheck</FillChecking>",
        "\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>",
        "\t\t<ChoiceParameterLinks/>",
        "\t\t<ChoiceParameters/>",
        "\t\t<QuickChoice>Auto</QuickChoice>",
        "\t\t<CreateOnInput>Auto</CreateOnInput>",
        "\t\t<ChoiceForm/>",
        "\t\t<LinkByType/>",
        "\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>",
    ] + (["\t\t<Use>ForItem</Use>"] if use else []) + [
        "\t\t<Indexing>DontIndex</Indexing>",
        "\t\t<FullTextSearch>Use</FullTextSearch>",
        "\t\t<DataHistory>Use</DataHistory>",
        "\t</Properties>",
        "</Attribute>",
    ]
    return "\n".join(t + x for x in lines)


def insert_after_last(text, close_tag, block, indent="\t\t\t"):
    """Insert `block` after the last `indent</close_tag>` line."""
    needle = "\n" + indent + "</%s>" % close_tag
    i = text.rfind(needle)
    assert i >= 0, close_tag
    j = i + len(needle)
    return text[:j] + "\n" + block + text[j:]


# ---------------------------------------------------------------------------------------------
# The changes. Each returns a short description; `INFO[name]` says what the matrix expects.
# ---------------------------------------------------------------------------------------------

CHANGES = {}


def change(name, family, summary):
    def deco(fn):
        CHANGES[name] = {"fn": fn, "family": family, "summary": summary}
        return fn
    return deco


@change("noop", "control: no change", "the unchanged tree (a control: nothing may differ)")
def c_noop(t):
    return {}


@change("attr", "descriptor: new attribute", "String(50) attribute ДемоНовыйРеквизит added to Catalog._ДемоПартнеры")
def c_attr(t):
    rel = "Catalogs/_ДемоПартнеры.xml"
    text = t.read(rel)
    block = attribute_block("ДемоНовыйРеквизит", "Демо новый реквизит", string_type(50))
    marker = "\t\t\t<TabularSection uuid=\""
    i = text.find(marker)
    assert i >= 0
    text = text[:i] + block + "\n" + text[i:]
    t.write(rel, text)
    return {"object": "Catalog._ДемоПартнеры"}


@change("ts", "descriptor: new tabular section", "tabular section ДемоНоваяТЧ (clone of the smallest one) on Catalog._ДемоКонтрагенты")
def c_ts(t):
    rel = "Catalogs/_ДемоКонтрагенты.xml"
    text = t.read(rel)
    secs = list(re.finditer(r'\t\t\t<TabularSection uuid="[^"]+">.*?\n\t\t\t</TabularSection>', text, re.S))
    src = min(secs, key=lambda m: len(m.group(0)))
    body = src.group(0)
    old_name = re.search(r"<Name>([^<]+)</Name>", body).group(1)
    body, _ = fresh_ids(body)
    body = body.replace("._ДемоКонтрагенты.%s" % old_name, "._ДемоКонтрагенты.ДемоНоваяТЧ")
    body = body.replace("<Name>%s</Name>" % old_name, "<Name>ДемоНоваяТЧ</Name>", 1)
    body = body.replace("<v8:content>%s</v8:content>" % old_name, "<v8:content>Демо новая ТЧ</v8:content>", 1)
    last = secs[-1]
    text = text[:last.end()] + "\n" + body + text[last.end():]
    t.write(rel, text)
    return {"object": "Catalog._ДемоКонтрагенты"}


@change("newcat", "new object", "Catalog ДемоНовыйСправочник (clone of Удалить_ДемоОбщиеСведения) + Configuration.xml entry")
def c_newcat(t):
    src = "Catalogs/Удалить_ДемоОбщиеСведения.xml"
    text = t.read(src)
    text, _ = fresh_ids(text)
    text = text.replace("Удалить_ДемоОбщиеСведения", "ДемоНовыйСправочник")
    text = text.replace("(не используется) Демо: Общие сведения", "Демо: Новый справочник")
    t.write("Catalogs/ДемоНовыйСправочник.xml", text)
    conf = t.read("Configuration.xml")
    names = re.findall(r"<Catalog>([^<]+)</Catalog>", conf)
    later = [n for n in names if n > "ДемоНовыйСправочник"]
    if later:
        key = "\t\t\t<Catalog>%s</Catalog>" % later[0]
        conf = conf.replace(key, "\t\t\t<Catalog>ДемоНовыйСправочник</Catalog>\n" + key, 1)
    else:
        last = "\t\t\t<Catalog>%s</Catalog>" % names[-1]
        conf = conf.replace(last, last + "\n\t\t\t<Catalog>ДемоНовыйСправочник</Catalog>", 1)
    t.write("Configuration.xml", conf)
    return {"object": "Catalog.ДемоНовыйСправочник"}


@change("syn", "descriptor: synonym", "synonym of Document._ДемоОприходованиеТоваров changed")
def c_syn(t):
    rel = "Documents/_ДемоОприходованиеТоваров.xml"
    text = t.read(rel)
    m = re.search(r"(<Properties>\s*<Name>_ДемоОприходованиеТоваров</Name>\s*<Synonym>\s*<v8:item>\s*<v8:lang>ru</v8:lang>\s*<v8:content>)([^<]*)(</v8:content>)", text)
    assert m
    text = text[:m.end(2)] + " (изменено)" + text[m.end(2):]
    t.write(rel, text)
    return {"object": "Document._ДемоОприходованиеТоваров"}


@change("prop", "descriptor: object property", "QuickChoice flipped on Catalog._ДемоГруппыДоступаПартнеров")
def c_prop(t):
    rel = "Catalogs/_ДемоГруппыДоступаПартнеров.xml"
    text = t.read(rel)
    m = re.search(r"\n\t\t\t<QuickChoice>(true|false)</QuickChoice>", text)
    assert m, "no object-level QuickChoice"
    flipped = "false" if m.group(1) == "true" else "true"
    text = text[:m.start(1)] + flipped + text[m.end(1):]
    t.write(rel, text)
    return {"object": "Catalog._ДемоГруппыДоступаПартнеров", "QuickChoice": "%s -> %s" % (m.group(1), flipped)}


@change("attrprop", "descriptor: attribute property", "FillChecking of an existing attribute of Catalog._ДемоФизическиеЛица set to ShowError")
def c_attrprop(t):
    rel = "Catalogs/_ДемоФизическиеЛица.xml"
    text = t.read(rel)
    m = re.search(r'(\t\t\t<Attribute uuid="[^"]+">.*?<FillChecking>)DontCheck(</FillChecking>)', text, re.S)
    assert m
    text = text[:m.start(0)] + m.group(1) + "ShowError" + m.group(2) + text[m.end(0):]
    t.write(rel, text)
    return {"object": "Catalog._ДемоФизическиеЛица"}


@change("attrdel", "descriptor: attribute removed", "the first attribute removed from Catalog._ДемоГруппыДоступаНоменклатуры")
def c_attrdel(t):
    rel = "Catalogs/_ДемоГруппыДоступаНоменклатуры.xml"
    text = t.read(rel)
    m = re.search(r'\t\t\t<Attribute uuid="[^"]+">.*?\n\t\t\t</Attribute>\n', text, re.S)
    assert m
    name = re.search(r"<Name>([^<]+)</Name>", m.group(0)).group(1)
    text = text[:m.start()] + text[m.end():]
    t.write(rel, text)
    return {"object": "Catalog._ДемоГруппыДоступаНоменклатуры", "removed": name}


@change("newform", "new owned object", "form ДемоНоваяФорма (clone of ФормаЭлемента) on Catalog._ДемоГруппыДоступаНоменклатуры")
def c_newform(t):
    owner = "Catalogs/_ДемоГруппыДоступаНоменклатуры"
    src = owner + "/Forms/ФормаЭлемента"
    desc = t.read(src + ".xml")
    desc, _ = fresh_ids(desc)
    desc = desc.replace("<Name>ФормаЭлемента</Name>", "<Name>ДемоНоваяФорма</Name>")
    desc = re.sub(r"(<v8:content>)[^<]*(</v8:content>)", r"\1Демо новая форма\2", desc, count=1)
    t.write(owner + "/Forms/ДемоНоваяФорма.xml", desc)
    body = t.read(src + "/Ext/Form.xml")
    t.write(owner + "/Forms/ДемоНоваяФорма/Ext/Form.xml", body)
    if t.exists(src + "/Ext/Form/Module.bsl"):
        t.write(owner + "/Forms/ДемоНоваяФорма/Ext/Form/Module.bsl", t.read(src + "/Ext/Form/Module.bsl"))
    text = t.read(owner + ".xml")
    key = "\t\t\t<Form>ФормаСписка</Form>"
    assert key in text
    text = text.replace(key, key + "\n\t\t\t<Form>ДемоНоваяФорма</Form>", 1)
    t.write(owner + ".xml", text)
    return {"object": "Catalog._ДемоГруппыДоступаНоменклатуры", "form": "ДемоНоваяФорма"}


@change("newtpl", "new owned object", "text template ДемоНовыйМакет (clone of ДатыПасха) on DataProcessor.ЗаполнениеКалендарныхГрафиков")
def c_newtpl(t):
    owner = "DataProcessors/ЗаполнениеКалендарныхГрафиков"
    src = owner + "/Templates/ДатыПасха"
    desc = t.read(src + ".xml")
    desc, _ = fresh_ids(desc)
    desc = desc.replace("<Name>ДатыПасха</Name>", "<Name>ДемоНовыйМакет</Name>")
    desc = re.sub(r"(<v8:content>)[^<]*(</v8:content>)", r"\1Демо новый макет\2", desc, count=1)
    t.write(owner + "/Templates/ДемоНовыйМакет.xml", desc)
    with open(os.path.join(t.base, src, "Ext", "Template.txt"), "rb") as f:
        t.write_raw(owner + "/Templates/ДемоНовыйМакет/Ext/Template.txt", f.read())
    text = t.read(owner + ".xml")
    tpls = re.findall(r"\t\t\t<Template>[^<]+</Template>", text)
    assert tpls
    text = text.replace(tpls[-1], tpls[-1] + "\n\t\t\t<Template>ДемоНовыйМакет</Template>", 1)
    t.write(owner + ".xml", text)
    return {"object": "DataProcessor.ЗаполнениеКалендарныхГрафиков", "template": "ДемоНовыйМакет"}


@change("predef", "body: predefined item", "predefined item ДемоНовыйЭлемент added to Catalog.СостоянияОригиналовПервичныхДокументов")
def c_predef(t):
    rel = "Catalogs/СостоянияОригиналовПервичныхДокументов/Ext/Predefined.xml"
    text = t.read(rel)
    item = "\n".join([
        '\t<Item id="%s">' % uuidlib.uuid4(),
        "\t\t<Name>ДемоНовыйЭлемент</Name>",
        "\t\t<Code>000000004</Code>",
        "\t\t<Description>Демо новый элемент</Description>",
        "\t\t<IsFolder>false</IsFolder>",
        "\t</Item>",
    ])
    i = text.rfind("</PredefinedData>")
    text = text[:i] + item + "\n" + text[i:]
    t.write(rel, text)
    return {"object": "Catalog.СостоянияОригиналовПервичныхДокументов"}


@change("predefedit", "body: predefined item", "Description of the predefined item Россия changed in Catalog.СтраныМира")
def c_predefedit(t):
    rel = "Catalogs/СтраныМира/Ext/Predefined.xml"
    text = t.read(rel)
    assert "<Description>РОССИЯ</Description>" in text
    text = text.replace("<Description>РОССИЯ</Description>", "<Description>РОССИЯ (изменено)</Description>", 1)
    t.write(rel, text)
    return {"object": "Catalog.СтраныМира"}


@change("predefdel", "body: predefined item removed", "predefined item ФормаНапечатана removed from Catalog.СостоянияОригиналовПервичныхДокументов")
def c_predefdel(t):
    rel = "Catalogs/СостоянияОригиналовПервичныхДокументов/Ext/Predefined.xml"
    text = t.read(rel)
    m = re.search(r'\t<Item id="[^"]+">\n\t\t<Name>ФормаНапечатана</Name>.*?\n\t</Item>\n', text, re.S)
    assert m
    text = text[:m.start()] + text[m.end():]
    t.write(rel, text)
    return {"object": "Catalog.СостоянияОригиналовПервичныхДокументов"}


@change("enumval", "descriptor: enum value", "enum value ДемоНовоеЗначение added to Enum.ТипыХраненияФайлов")
def c_enumval(t):
    rel = "Enums/ТипыХраненияФайлов.xml"
    text = t.read(rel)
    block = "\n".join([
        '\t\t\t<EnumValue uuid="%s">' % uuidlib.uuid4(),
        "\t\t\t\t<Properties>",
        "\t\t\t\t\t<Name>ДемоНовоеЗначение</Name>",
        synonym_xml("Демо новое значение", 5),
        "\t\t\t\t\t<Comment/>",
        "\t\t\t\t</Properties>",
        "\t\t\t</EnumValue>",
    ])
    text = insert_after_last(text, "EnumValue", block)
    t.write(rel, text)
    return {"object": "Enum.ТипыХраненияФайлов"}


@change("rights", "body: role rights", "View of Catalog.ВнешниеПользователи.Command.ВнешнийДоступ switched off in Role.ДобавлениеИзменениеВнешнихПользователей")
def c_rights(t):
    rel = "Roles/ДобавлениеИзменениеВнешнихПользователей/Ext/Rights.xml"
    text = t.read(rel)
    m = re.search(r"(<name>Catalog\.ВнешниеПользователи\.Command\.ВнешнийДоступ</name>\s*<right>\s*<name>View</name>\s*<value>)true(</value>)", text)
    assert m
    text = text[:m.start(0)] + m.group(1) + "false" + m.group(2) + text[m.end(0):]
    t.write(rel, text)
    return {"object": "Role.ДобавлениеИзменениеВнешнихПользователей"}


@change("subsys", "descriptor: subsystem content", "Catalog.Заметки added to the content of Subsystem._ДемоНачальнаяСтраница")
def c_subsys(t):
    rel = "Subsystems/_ДемоНачальнаяСтраница.xml"
    text = t.read(rel)
    key = "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">Report._ДемоФайлыВспомогательный</xr:Item>"
    assert key in text
    text = text.replace(key, key + "\n\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">Catalog.Заметки</xr:Item>", 1)
    t.write(rel, text)
    return {"object": "Subsystem._ДемоНачальнаяСтраница"}


@change("nestsub", "descriptor: nested subsystem content", "Catalog.Заметки added to the content of the nested Subsystem.КонтрольВеденияУчета")
def c_nestsub(t):
    rel = "Subsystems/СтандартныеПодсистемы/Subsystems/КонтрольВеденияУчета.xml"
    text = t.read(rel)
    m = re.search(r"\t\t\t<Content>\n(.*?)\n\t\t\t</Content>", text, re.S)
    if m:
        items = m.group(1).split("\n")
        new = items + ["\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">Catalog.Заметки</xr:Item>"]
        text = text[:m.start(1)] + "\n".join(new) + text[m.end(1):]
    else:
        assert "<Content/>" in text, "no Content in the nested subsystem"
        text = text.replace("<Content/>", "<Content>\n\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">Catalog.Заметки</xr:Item>\n\t\t\t</Content>", 1)
    t.write(rel, text)
    return {"object": "Subsystem.КонтрольВеденияУчета (nested)"}


@change("ci", "body: command interface", "visibility of a command flipped in Subsystem._ДемоАнкетирование/Ext/CommandInterface.xml")
def c_ci(t):
    rel = "Subsystems/_ДемоАнкетирование/Ext/CommandInterface.xml"
    text = t.read(rel)
    m = re.search(r"(<Command name=\"[^\"]+\">\s*<Visibility>\s*<xr:Common>)(true|false)(</xr:Common>)", text)
    assert m
    flipped = "false" if m.group(2) == "true" else "true"
    text = text[:m.start(2)] + flipped + text[m.end(2):]
    t.write(rel, text)
    return {"object": "Subsystem._ДемоАнкетирование", "flip": "%s -> %s" % (m.group(2), flipped)}


@change("module", "body: module (control)", "a comment line appended to CommonModule._ДемоЗаметки/Ext/Module.bsl")
def c_module(t):
    rel = "CommonModules/_ДемоЗаметки/Ext/Module.bsl"
    text = t.read(rel)
    if not text.endswith("\n"):
        text += "\n"
    text += "// import-lab: module-only change\n"
    t.write(rel, text)
    return {"object": "CommonModule._ДемоЗаметки"}


@change("confver", "descriptor: configuration property", "Configuration <Version> bumped 3.1.11.466 -> 3.1.11.467")
def c_confver(t):
    rel = "Configuration.xml"
    text = t.read(rel)
    assert "<Version>3.1.11.466</Version>" in text
    text = text.replace("<Version>3.1.11.466</Version>", "<Version>3.1.11.467</Version>", 1)
    t.write(rel, text)
    return {"object": "Configuration"}


@change("formdel", "owned object removed", "form ВсеЗаметки removed from Catalog.Заметки (files and the <Form> entry)")
def c_formdel(t):
    owner = "Catalogs/Заметки"
    t.delete(owner + "/Forms/ВсеЗаметки.xml")
    t.delete(owner + "/Forms/ВсеЗаметки")
    text = t.read(owner + ".xml")
    key = "\t\t\t<Form>ВсеЗаметки</Form>\n"
    assert key in text
    text = text.replace(key, "", 1)
    t.write(owner + ".xml", text)
    return {"object": "Catalog.Заметки", "form": "ВсеЗаметки"}


@change("tpldel", "owned object removed", "template ДатыПасха removed from DataProcessor.ЗаполнениеКалендарныхГрафиков (files and the <Template> entry)")
def c_tpldel(t):
    owner = "DataProcessors/ЗаполнениеКалендарныхГрафиков"
    t.delete(owner + "/Templates/ДатыПасха.xml")
    t.delete(owner + "/Templates/ДатыПасха")
    text = t.read(owner + ".xml")
    key = "\t\t\t<Template>ДатыПасха</Template>\n"
    assert key in text
    text = text.replace(key, "", 1)
    t.write(owner + ".xml", text)
    return {"object": "DataProcessor.ЗаполнениеКалендарныхГрафиков", "template": "ДатыПасха"}


@change("formdel_demo", "owned object removed", "non-default demo form ДокументыКонтрагентов removed from Catalog._ДемоКонтрагенты")
def c_formdel_demo(t):
    owner = "Catalogs/_ДемоКонтрагенты"
    name = "ДокументыКонтрагентов"
    t.delete(owner + "/Forms/" + name + ".xml")
    t.delete(owner + "/Forms/" + name)
    text = t.read(owner + ".xml")
    key = "\t\t\t<Form>" + name + "</Form>\n"
    assert key in text
    t.write(owner + ".xml", text.replace(key, "", 1))
    return {"object": "Catalog._ДемоКонтрагенты", "form": name}


@change("tpldel_demo", "owned object removed", "demo template Логотип removed from DataProcessor._ДемоШаблонСообщенияДляЗаказаПокупателя")
def c_tpldel_demo(t):
    owner = "DataProcessors/_ДемоШаблонСообщенияДляЗаказаПокупателя"
    name = "Логотип"
    t.delete(owner + "/Templates/" + name + ".xml")
    t.delete(owner + "/Templates/" + name)
    text = t.read(owner + ".xml")
    key = "\t\t\t<Template>" + name + "</Template>\n"
    assert key in text
    t.write(owner + ".xml", text.replace(key, "", 1))
    return {"object": "DataProcessor._ДемоШаблонСообщенияДляЗаказаПокупателя", "template": name}


@change("catfile", "object file removed, listing kept", "Catalogs/Удалить_ДемоОбщиеСведения.xml removed but Configuration.xml still lists it")
def c_catfile(t):
    t.delete("Catalogs/Удалить_ДемоОбщиеСведения.xml")
    if t.exists("Catalogs/Удалить_ДемоОбщиеСведения"):
        t.delete("Catalogs/Удалить_ДемоОбщиеСведения")
    return {"object": "Catalog.Удалить_ДемоОбщиеСведения"}


@change("catdel", "object removed", "Catalog.Удалить_ДемоОбщиеСведения removed (file and Configuration.xml entry)")
def c_catdel(t):
    t.delete("Catalogs/Удалить_ДемоОбщиеСведения.xml")
    if t.exists("Catalogs/Удалить_ДемоОбщиеСведения"):
        t.delete("Catalogs/Удалить_ДемоОбщиеСведения")
    conf = t.read("Configuration.xml")
    key = "\t\t\t<Catalog>Удалить_ДемоОбщиеСведения</Catalog>\n"
    assert key in conf
    conf = conf.replace(key, "", 1)
    t.write("Configuration.xml", conf)
    return {"object": "Catalog.Удалить_ДемоОбщиеСведения"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["list", "apply", "reset", "info"])
    ap.add_argument("names", nargs="?", default="")
    ap.add_argument("--base")
    ap.add_argument("--work")
    args = ap.parse_args()
    if args.cmd == "list":
        for name, c in CHANGES.items():
            print("%-9s %-38s %s" % (name, c["family"], c["summary"]))
        return
    if args.cmd == "info":
        print(json.dumps({k: {"family": v["family"], "summary": v["summary"]} for k, v in CHANGES.items() if k in args.names.split(",")},
                         ensure_ascii=False, indent=1))
        return
    t = Tree(args.base, args.work)
    if args.cmd == "reset":
        t.reset()
        print("reset")
        return
    out = {}
    for name in args.names.split(","):
        out[name] = CHANGES[name]["fn"](t)
    print(json.dumps(out, ensure_ascii=False))


if __name__ == "__main__":
    main()
