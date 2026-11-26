const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const {
  TreeModel,
  XmlSource,
  DatabaseSource,
  descriptor,
  relativePath,
  readResult,
} = require("../out/configuration-tree");
const signal = () => new AbortController().signal;

test("модуль встроенной команды из штатного описания виден у команды", async () => {
  const bytes = await fs.readFile(
    path.resolve(
      __dirname,
      "../../../tests/fixtures/native-evidence/8.3.27.2214/task-assignee/native/Tasks/ЗадачаИсполнителя.xml",
    ),
  );
  const owner = {
    id: "Task.ЗадачаИсполнителя",
    fullName: "Task.ЗадачаИсполнителя",
    label: "ЗадачаИсполнителя",
    kind: "Task",
    type: "object",
    path: "Tasks/ЗадачаИсполнителя.xml",
  };
  const modulePath =
    "Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl";
  const groups = descriptor(bytes, owner, [modulePath]);
  const commands = groups.find((node) => node.kind === "Command");
  assert.equal(commands.children.length, 4);
  const command = commands.children.find((node) => node.label === "ВсеЗадачи");
  assert.equal(command.children[0].path, modulePath);
  assert.equal(command.children[0].type, "file");
  const temporary = await fs.mkdtemp(
    path.join(os.tmpdir(), "ibcmd-command-tree-"),
  );
  try {
    await fs.mkdir(path.dirname(path.join(temporary, modulePath)), {
      recursive: true,
    });
    await fs.writeFile(path.join(temporary, owner.path), bytes);
    await fs.writeFile(
      path.join(temporary, modulePath),
      "// Проверка модуля команды\r\n",
    );
    const local = await new XmlSource(temporary).children(owner, signal());
    assert.equal(
      local.find((node) => node.kind === "Command").children[0].children[0]
        .path,
      modulePath,
    );
  } finally {
    await fs.rm(temporary, { recursive: true, force: true });
  }
});
const xml = (kind, content) =>
  Buffer.from(
    `<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core"><${kind}>${content}</${kind}></MetaDataObject>`,
  );

test("сохранённая штатная выгрузка: состав дерева, модуль и поиск", async () => {
  const source = new XmlSource(
    path.resolve(__dirname, "../../../tests/fixtures/file-infobase/tiny-xml"),
  );
  const model = new TreeModel(source);
  try {
    const [root] = await model.children();
    const [general] = await model.children(root);
    assert.equal(general.label, "Общие");
    const groups = await model.children(general);
    const modules = groups.find((n) => n.kind === "CommonModule");
    const [module] = await model.children(modules);
    assert.equal(module.fullName, "CommonModule.ФайловаяБазаПроверка");
    const [file] = await model.children(module);
    assert.equal(
      file.path,
      "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl",
    );
    assert.equal(file.parent, module);
    const content = await source.read(file.path, signal());
    assert.deepEqual(
      content,
      await fs.readFile(path.join(source.directory, file.path)),
    );
    assert.equal((await model.search("файловая", signal())).length, 1);
    assert.equal((await model.children(modules))[0], module);
  } finally {
    model.dispose();
  }
});

test("вложенные реквизиты, табличные части, формы, синонимы и двоичные макеты", () => {
  const owner = {
    id: "Catalog.Товары",
    fullName: "Catalog.Товары",
    label: "Товары",
    kind: "Catalog",
    type: "object",
    path: "Catalogs/Товары.xml",
  };
  const result = descriptor(
    xml(
      "Catalog",
      `<Properties><Name>Товары</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Номенклатура &amp; услуги</v8:content></v8:item></Synonym></Properties><ChildObjects><Attribute><Properties><Name>Цена</Name></Properties></Attribute><TabularSection><Properties><Name>Строки</Name></Properties><ChildObjects><Attribute><Properties><Name>Количество</Name></Properties></Attribute></ChildObjects></TabularSection><Form>ФормаЭлемента</Form></ChildObjects>`,
    ),
    owner,
    [
      "Catalogs/Товары/Ext/ObjectModule.bsl",
      "Catalogs/Товары/Forms/ФормаЭлемента/Ext/Form.xml",
    ],
  );
  assert.equal(owner.synonym, "Номенклатура & услуги");
  assert.deepEqual(
    result.map((n) => n.kind),
    ["Attribute", "TabularSection", "Form", "file"],
  );
  assert.equal(result[0].children[0].type, "property");
  assert.equal(
    result[1].children[0].children[0].children[0].label,
    "Количество",
  );
  assert.equal(
    result[2].children[0].path,
    "Catalogs/Товары/Forms/ФормаЭлемента.xml",
  );
  assert.equal(result[3].path, "Catalogs/Товары/Ext/ObjectModule.bsl");
});

test("база читается по уровням; повторные и одновременные раскрытия используют кэш", async () => {
  const calls = [];
  const rpc = {
    request: async (method, params) => {
      calls.push({ method, params });
      return {
        nodes: params.node
          ? [
              {
                id: "Configuration/Catalogs",
                label: "Catalogs",
                kind: "Catalog",
                nodeType: "group",
              },
            ]
          : [
              {
                id: "Configuration",
                label: "База",
                kind: "Configuration",
                nodeType: "configuration",
                path: "Configuration.xml",
              },
            ],
      };
    },
  };
  const model = new TreeModel(new DatabaseSource(rpc, "/work", "demo"));
  const [[root], again] = await Promise.all([
    model.children(),
    model.children(),
  ]);
  assert.equal(root, again[0]);
  assert.equal(calls.length, 1);
  await model.children(root);
  await model.children(root);
  assert.equal(calls.length, 2);
  assert.ok(calls.every((c) => c.method === "tree/children"));
  model.dispose();
});

test("устаревший ответ после закрытия модели не возвращает узлы", async () => {
  let finish;
  const source = {
    children: () => new Promise((resolve) => (finish = resolve)),
  };
  const model = new TreeModel(source);
  const pending = model.children();
  model.dispose();
  finish([{ id: "old" }]);
  assert.deepEqual(await pending, []);
});

test("ошибка чтения допускает повтор, цикл и повторные узлы отклоняются", async () => {
  let count = 0;
  const model = new TreeModel({
    children: async () => {
      if (++count === 1) throw Error("сбой");
      return [{ id: "a" }, { id: "a" }];
    },
  });
  await assert.rejects(model.children());
  await assert.rejects(model.children(), /Повторяющийся/);
  assert.equal(count, 2);
  const cyclic = new TreeModel({ children: async () => [{ id: "a" }] });
  const [root] = await cyclic.children();
  await assert.rejects(cyclic.children(root), /Циклическое/);
  model.dispose();
  cyclic.dispose();
});

test("поиск по синониму и отмена поиска", async () => {
  const root = { id: "r", label: "Имя", type: "object" };
  const model = new TreeModel({
    children: async (node) => {
      if (!node) return [root];
      node.synonym = "Представление";
      return [];
    },
  });
  assert.deepEqual(await model.search("представ", signal()), [root]);
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(model.search("a", abort.signal), /отменён/);
  model.dispose();
});

test("пути, повреждённый XML, DTD и неизвестные кодировки отклоняются", () => {
  for (const file of ["../x", "/root", "a\\b", "C:/x", "a//b", "a/../b"])
    assert.throws(() => relativePath(file));
  const owner = { kind: "Catalog", path: "Catalogs/Товар.xml" };
  assert.throws(() =>
    descriptor(Buffer.from("<!DOCTYPE x><MetaDataObject/>"), owner, []),
  );
  assert.throws(() =>
    descriptor(Buffer.from("<MetaDataObject><Catalog>"), owner, []),
  );
  assert.throws(() =>
    readResult(
      { path: "a", exists: true, encoding: "base64", content: "!!" },
      "a",
    ),
  );
  assert.deepEqual(
    readResult(
      { path: "a", exists: true, encoding: "base64", content: "AP8=" },
      "a",
    ),
    Buffer.from([0, 255]),
  );
});

test("посторонние пространства имён не скрывают соседние реквизиты и файлы", async () => {
  for (const version of ["2.17", "2.20", "2.21"]) {
    const source = new XmlSource(
      path.resolve(__dirname, "../../../tests/fixtures/xml", version),
    );
    const owner = {
      id: "Catalog.Products",
      fullName: "Catalog.Products",
      kind: "Catalog",
      type: "object",
      label: "Products",
      path: "Catalogs/Products.xml",
    };
    const children = await source.children(owner, signal());
    assert.equal(
      children.find((n) => n.kind === "Attribute").children[0].label,
      "Sku",
    );
    assert.equal(
      children.find((n) => n.kind === "TabularSection").children[0].label,
      "Lines",
    );
    const modulePath = "Catalogs/Products/Ext/ObjectModule.bsl";
    assert.ok(
      descriptor(await source.read(owner.path, signal()), owner, [
        modulePath,
      ]).some((n) => n.path === modulePath),
    );
    const model = new TreeModel({
      children: (node, abort) =>
        node ? source.children(node, abort) : Promise.resolve([owner]),
    });
    try {
      assert.equal((await model.search("Quantity", signal())).length, 1);
    } finally {
      model.dispose();
    }
  }

  const owner = { kind: "Catalog", path: "Catalogs/Товары.xml" };
  const bytes = xml(
    "Catalog",
    '<ChildObjects xmlns:f="urn:посторонние"><f:Attribute><f:Properties><f:Name>Чужой</f:Name></f:Properties></f:Attribute><Attribute><f:Properties/><Properties><Name>Свой</Name></Properties></Attribute></ChildObjects>',
  );
  assert.equal(descriptor(bytes, owner, [])[0].children[0].label, "Свой");
  assert.throws(
    () =>
      descriptor(
        xml("Catalog", "<ChildObjects><Unknown>Имя</Unknown></ChildObjects>"),
        owner,
        [],
      ),
    /Неизвестная коллекция/,
  );
});

test("отмена последнего чтения поиска передаётся серверу и позволяет повтор", async () => {
  let finish;
  let readSignal;
  let calls = 0;
  const source = new DatabaseSource(
    {
      request: async (_method, _params, abort) => {
        calls++;
        if (calls > 1) return { nodes: [] };
        readSignal = abort;
        return new Promise((resolve) => {
          finish = resolve;
        });
      },
    },
    "/work",
    "demo",
  );
  const model = new TreeModel(source);
  const abort = new AbortController();
  const search = model.search("Имя", abort.signal);
  abort.abort();
  await assert.rejects(search, /отменён/);
  assert.equal(readSignal.aborted, true);
  assert.deepEqual(await model.search("Имя", signal()), []);
  finish({ nodes: [] });
  await new Promise((resolve) => setImmediate(resolve));
  await model.children();
  assert.equal(calls, 2);
  model.dispose();
});

test("отмена поиска не отменяет совместное раскрытие узла", async () => {
  let finish;
  let readSignal;
  const model = new TreeModel({
    children: (_node, abort) => {
      readSignal = abort;
      return new Promise((resolve) => {
        finish = resolve;
      });
    },
  });
  const abort = new AbortController();
  const search = model.search("Имя", abort.signal);
  const expansion = model.children();
  abort.abort();
  await assert.rejects(search, /отменён/);
  assert.equal(readSignal.aborted, false);
  finish([]);
  assert.deepEqual(await expansion, []);
  model.dispose();
});

test("отмена после последнего ответа не показывает результаты поиска", async () => {
  const abort = new AbortController();
  const model = new TreeModel({
    children: async () => {
      abort.abort();
      return [];
    },
  });
  await assert.rejects(model.search("Имя", abort.signal), /отменён/);
  model.dispose();
});

test("XML-источник не читает ссылку за пределы каталога", async (t) => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "ibcmd-tree-"));
  try {
    await fs.mkdir(path.join(dir, "root"));
    await fs.mkdir(path.join(dir, "outside"));
    await fs.writeFile(
      path.join(dir, "outside", "secret.xml"),
      "закрытые данные",
    );
    await fs.symlink(
      path.join(dir, "outside"),
      path.join(dir, "root", "link"),
      "junction",
    );
    const source = new XmlSource(path.join(dir, "root"));
    await assert.rejects(
      source.read("link/secret.xml", signal()),
      /вне каталога/,
    );
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("foreign preservation children keep valid metadata siblings and files visible", async () => {
  for (const version of ["2.17", "2.20", "2.21"]) {
    const bytes = await fs.readFile(
      path.resolve(
        __dirname,
        `../../../tests/fixtures/xml/${version}/Catalogs/Products.xml`,
      ),
    );
    const owner = {
      id: "Catalog.Products",
      fullName: "Catalog.Products",
      label: "Products",
      kind: "Catalog",
      type: "object",
      path: "Catalogs/Products.xml",
    };
    const modulePath = "Catalogs/Products/Ext/ObjectModule.bsl";
    const nodes = descriptor(bytes, owner, [modulePath]);
    assert.deepEqual(
      nodes.map((n) => n.kind),
      ["Attribute", "TabularSection", "file"],
    );
    assert.equal(nodes[0].children[0].label, "Sku");
    assert.equal(nodes[1].children[0].label, "Lines");
    assert.equal(nodes[2].path, modulePath);
    const sameName = bytes
      .toString()
      .replace(
        "<ChildObjects>",
        "<ChildObjects><f:Attribute>ignored/foreign</f:Attribute>",
      );
    assert.equal(descriptor(Buffer.from(sameName), owner, []).length, 2);
  }
});

test("cancelling a pending database search aborts its RPC and discards final results", async () => {
  let loaded, finish, pendingSignal;
  const loading = new Promise((resolve) => (loaded = resolve));
  const rpc = {
    request: async (method, params, signal) => {
      if (method === "tree/children")
        return {
          nodes: [
            {
              id: "Catalog.Test",
              label: "Test",
              kind: "Catalog",
              nodeType: "object",
              fullName: "Catalog.Test",
              path: "Catalogs/Test.xml",
            },
          ],
        };
      pendingSignal = signal;
      loaded();
      return new Promise(
        (resolve) =>
          (finish = () =>
            resolve({
              path: params.path,
              exists: true,
              encoding: "utf-8",
              content: xml("Catalog", "<ChildObjects/>").toString(),
              files: [],
            })),
      );
    },
  };
  const model = new TreeModel(new DatabaseSource(rpc, "/work", "demo"));
  const abort = new AbortController();
  const pending = model.search("test", abort.signal);
  const rejected = assert.rejects(pending, /отмен/);
  try {
    await loading;
    abort.abort();
    assert.equal(
      pendingSignal.aborted,
      true,
      "the source RPC must receive cancellation",
    );
  } finally {
    finish();
    await rejected;
    model.dispose();
  }
});

test("cancelling search leaves a shared tree expansion usable", async () => {
  let finish, readSignal;
  const model = new TreeModel({
    children: (_, signal) => {
      readSignal = signal;
      return new Promise((resolve) => (finish = resolve));
    },
  });
  const expansion = model.children();
  const abort = new AbortController();
  const search = model.search("test", abort.signal);
  const rejected = assert.rejects(search, /отмен/);
  abort.abort();
  assert.equal(readSignal.aborted, false);
  finish([]);
  await rejected;
  assert.deepEqual(await expansion, []);
  model.dispose();
});

test("large database source reads use a finite extended deadline and retain cancellation", async () => {
  const calls = [];
  const abort = new AbortController();
  const source = new DatabaseSource(
    {
      request: async (method, params, signal, timeout) => {
        calls.push({ method, signal, timeout });
        return {
          path: params.path,
          exists: true,
          encoding: "utf-8",
          files: [],
          content: xml(
            params.path === "Configuration.xml" ? "Configuration" : "Catalog",
            "<ChildObjects/>",
          ).toString(),
        };
      },
    },
    "/work",
    "demo",
  );
  await source.read("Catalogs/Test.xml", abort.signal);
  await source.children(
    { type: "object", kind: "Catalog", path: "Catalogs/Test.xml" },
    abort.signal,
  );
  await source.children(
    { type: "group", kind: "ConfigurationFiles" },
    abort.signal,
  );
  for (const call of calls) {
    assert.equal(call.method, "source/read");
    assert.equal(call.signal, abort.signal);
    assert.ok(Number.isFinite(call.timeout) && call.timeout >= 600000);
  }
});

test("ordinary database tree reads retain the default deadline", async () => {
  const requests = [];
  const source = new DatabaseSource(
    {
      request: async (...args) => {
        requests.push(args);
        return { nodes: [] };
      },
    },
    "/source",
    "test",
  );
  const signal = new AbortController().signal;
  await source.children(undefined, signal);
  assert.equal(requests.length, 1);
  assert.equal(requests[0][0], "tree/children");
  assert.equal(requests[0][2], signal);
  assert.equal(requests[0][3], undefined);
});
