"""Build the adopted-subscription fixture with the platform: a base
configuration with an event subscription (a catalog as its source, a common
module handler), and an extension adopting it and adding its own catalog to
the source.

    python make_adopted_subscription_fixture.py

Output: tests/fixtures/external/adopted/subscription/{input.cfe, EventSubscriptions/Подписка.xml}
"""
import os, shutil, sys, tempfile
import local_paths  # noqa: E402
sys.path.insert(0, local_paths.ONECDEC_TOOLS)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8dump
import make_adopted_fixtures as adopted
import make_adopted_properties_fixture as props

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
EXE, EXT, write, md = adopted.EXE, adopted.EXT, adopted.write, props.md
OWN = 'ТестРасширение_Свой'


def uid(side, n):
    return '%s8000000-0000-4000-8000-%012d' % (side, n)


def catalog(uuid, name, seed):
    cats = [('Catalog' + a, a) for a in ['Object', 'Ref', 'Selection', 'List', 'Manager']]
    return md('\t<Catalog uuid="%s">\n\t\t<InternalInfo>\n%s\n\t\t</InternalInfo>\n\t\t<Properties>\n\t\t\t<Name>%s</Name>\n'
              '\t\t\t<Synonym/>\n\t\t\t<Comment/>\n\t\t</Properties>\n\t\t<ChildObjects/>\n\t</Catalog>'
              % (uuid, props.generated('Catalog', name, seed, cats), name))


def subscription(uuid, body):
    return md('\t<EventSubscription uuid="%s">\n%s\t</EventSubscription>' % (uuid, body))


def main():
    work = tempfile.mkdtemp(prefix='adopted-subscription-')
    log = os.path.join(work, 'platform.log'); ib = os.path.join(work, 'ib')
    base = os.path.join(work, 'base'); shutil.copytree(os.path.join(adopted.SRC, 'cfe_base'), base)
    ext = os.path.join(work, 'ext'); shutil.copytree(os.path.join(adopted.SRC, EXT), ext)
    write(os.path.join(base, 'Catalogs', 'Справочник.xml'), catalog(uid('b', 1), 'Справочник', 'c8000001'))
    write(os.path.join(base, 'CommonModules', 'Обработчики.xml'), md(
        '\t<CommonModule uuid="%s">\n\t\t<Properties>\n\t\t\t<Name>Обработчики</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n'
        '\t\t\t<Global>false</Global>\n\t\t\t<ClientManagedApplication>false</ClientManagedApplication>\n\t\t\t<Server>true</Server>\n'
        '\t\t\t<ExternalConnection>false</ExternalConnection>\n\t\t\t<ClientOrdinaryApplication>false</ClientOrdinaryApplication>\n'
        '\t\t\t<ServerCall>false</ServerCall>\n\t\t\t<Privileged>false</Privileged>\n\t\t\t<ReturnValuesReuse>DontUse</ReturnValuesReuse>\n'
        '\t\t</Properties>\n\t</CommonModule>' % uid('b', 2)))
    write(os.path.join(base, 'CommonModules', 'Обработчики', 'Ext', 'Module.bsl'),
          'Процедура ПередЗаписью(Источник, Отказ) Экспорт\nКонецПроцедуры\n', True)
    write(os.path.join(base, 'EventSubscriptions', 'Подписка.xml'), subscription(uid('b', 3),
          '\t\t<Properties>\n\t\t\t<Name>Подписка</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n\t\t\t<Source>\n'
          '\t\t\t\t<v8:Type>cfg:CatalogObject.Справочник</v8:Type>\n\t\t\t</Source>\n\t\t\t<Event>BeforeWrite</Event>\n'
          '\t\t\t<Handler>CommonModule.Обработчики.ПередЗаписью</Handler>\n\t\t</Properties>\n'))
    props.add_children(os.path.join(base, 'Configuration.xml'), '<Language>Русский</Language>',
                       [('CommonModule', 'Обработчики'), ('Catalog', 'Справочник'), ('EventSubscription', 'Подписка')])
    write(os.path.join(ext, 'Catalogs', OWN + '.xml'), catalog(uid('e', 1), OWN, 'd8000001'))
    write(os.path.join(ext, 'EventSubscriptions', 'Подписка.xml'), subscription(uid('e', 3),
          '\t\t<InternalInfo/>\n\t\t<Properties>\n\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n\t\t\t<Name>Подписка</Name>\n'
          '\t\t\t<Comment/>\n\t\t\t<ExtendedConfigurationObject>%s</ExtendedConfigurationObject>\n\t\t\t<Source>\n'
          '\t\t\t\t<v8:Type>cfg:CatalogObject.%s</v8:Type>\n\t\t\t</Source>\n\t\t</Properties>\n' % (uid('b', 3), OWN)))
    props.add_children(os.path.join(ext, 'Configuration.xml'), '<CommonModule>ТестРасширение_Модуль</CommonModule>',
                       [('Catalog', OWN), ('EventSubscription', 'Подписка')])
    v8dump._run(EXE, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', base], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/UpdateDBCfg'], log)
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/LoadConfigFromFiles', ext, '-Extension', EXT], log)
    dst = os.path.join(REPO, 'tests', 'fixtures', 'external', 'adopted', 'subscription')
    shutil.rmtree(dst, ignore_errors=True); os.makedirs(os.path.join(dst, 'EventSubscriptions'))
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpCfg', os.path.join(dst, 'input.cfe'), '-Extension', EXT], log)
    dump = os.path.join(work, 'dump')
    v8dump._run(EXE, ['DESIGNER', '/F', ib, '/DumpConfigToFiles', dump, '-Extension', EXT], log)
    rel = os.path.join('EventSubscriptions', 'Подписка.xml')
    shutil.copy(os.path.join(dump, rel), os.path.join(dst, rel))
    print(open(os.path.join(dst, rel), encoding='utf-8-sig').read().split('version="2.20">')[1])
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
