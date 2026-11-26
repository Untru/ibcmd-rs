const { test } = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
const core = require("../out/configuration-tree");
function load() {
  class FileSystemError extends Error {}
  for (const code of [
    "FileNotFound",
    "Unavailable",
    "FileNotADirectory",
    "NoPermissions",
  ])
    FileSystemError[code] = () =>
      Object.assign(new FileSystemError(code), { code });
  class Uri {
    constructor(value) {
      Object.assign(this, value);
    }
    toString() {
      return JSON.stringify({
        scheme: this.scheme,
        path: this.path,
        query: this.query,
      });
    }
    static from(value) {
      return new Uri(value);
    }
    static parse(value) {
      return new Uri(JSON.parse(value));
    }
  }
  const vscode = {
    Uri,
    FileSystemError,
    FileChangeType: { Deleted: 3 },
    FileType: { File: 1 },
    FilePermission: { Readonly: 1 },
    Disposable: class {
      dispose() {}
    },
    EventEmitter: class {
      event = () => ({ dispose() {} });
      fire() {}
      dispose() {}
    },
    workspace: { isTrusted: true },
  };
  const module = { exports: {} };
  vm.runInNewContext(
    fs.readFileSync(path.join(__dirname, "../out/database-files.js"), "utf8"),
    {
      module,
      exports: module.exports,
      AbortController,
      Uint8Array,
      Buffer,
      require: (id) =>
        id === "vscode"
          ? vscode
          : id === "./configuration-tree"
            ? core
            : require(id),
    },
  );
  return { ...module.exports, vscode };
}

test("виртуальный файл сохраняет точные байты и отказывает во всех изменениях", async () => {
  const { DatabaseFiles, vscode } = load();
  const files = new DatabaseFiles();
  const bytes = Buffer.from([239, 187, 191, 0, 255, 13, 10]);
  const source = new core.DatabaseSource(
    {
      request: async () => ({
        path: "Templates/Макет/Ext/Template.bin",
        exists: true,
        encoding: "base64",
        content: bytes.toString("base64"),
      }),
    },
    "/work",
    "База",
  );
  const uri = files.uri(source, "Templates/Макет/Ext/Template.bin");
  assert.deepEqual(await files.readFile(uri), bytes);
  const stat = await files.stat(uri);
  assert.equal(stat.size, bytes.length);
  assert.equal(stat.permissions, vscode.FilePermission.Readonly);
  for (const method of ["writeFile", "delete", "rename", "createDirectory"])
    assert.throws(() => files[method](uri), { code: "NoPermissions" });
  files.dispose();
});

test("неизвестные, отсутствующие и устаревшие файлы не подменяются другой базой", async () => {
  const { DatabaseFiles, vscode } = load();
  const files = new DatabaseFiles();
  const source = { read: async () => undefined };
  const uri = files.uri(source, "Configuration.xml");
  await assert.rejects(files.readFile(uri), { code: "FileNotFound" });
  const empty = files.uri(source, "Configuration.xml", true);
  assert.equal((await files.readFile(empty)).byteLength, 0);
  await assert.rejects(
    files.readFile(vscode.Uri.from({ scheme: "ibcmd-rs", path: "/unknown" })),
    { code: "FileNotFound" },
  );
  files.invalidate();
  const next = files.uri(
    { read: async () => Buffer.from("новая база") },
    "Configuration.xml",
  );
  assert.notEqual(next.toString(), uri.toString());
  await assert.rejects(files.readFile(uri), { code: "FileNotFound" });
  assert.equal((await files.readFile(next)).toString(), "новая база");
  files.dispose();
});

test("смена источника отменяет чтение; запросы одного файла объединяются", async () => {
  const { DatabaseFiles } = load();
  const files = new DatabaseFiles();
  let finish,
    signal,
    calls = 0;
  const source = {
    read: async (_, s) => {
      calls++;
      signal = s;
      return new Promise((resolve) => (finish = resolve));
    },
  };
  const uri = files.uri(source, "Ext/Module.bsl");
  const a = files.readFile(uri),
    b = files.readFile(uri);
  assert.equal(calls, 1);
  files.invalidate();
  assert.equal(signal.aborted, true);
  finish(Buffer.from("прежние данные"));
  await assert.rejects(a, { code: "Unavailable" });
  await assert.rejects(b, { code: "Unavailable" });
  files.dispose();
});

test("повторное открытие адреса во время чтения сохраняет результат для обоих вызовов", async () => {
  const { DatabaseFiles } = load();
  const files = new DatabaseFiles();
  let finish;
  let calls = 0;
  const source = {
    read: async () => {
      calls++;
      return new Promise((resolve) => {
        finish = resolve;
      });
    },
  };
  const first = files.readFile(files.uri(source, "Ext/Module.bsl"));
  const second = files.readFile(files.uri(source, "Ext/Module.bsl"));
  assert.equal(calls, 1);
  assert.equal(
    (await files.readFile(files.uri(source, "Ext/Module.bsl", true)))
      .byteLength,
    0,
  );
  assert.throws(
    () => files.uri({ read: source.read }, "Ext/Module.bsl"),
    /Источник/,
  );
  const bytes = Buffer.from("Точное содержимое\r\n");
  finish(bytes);
  assert.deepEqual(await first, bytes);
  assert.deepEqual(await second, bytes);
  files.dispose();
});

test("сравнение группы запрашивает также целиком местные объекты и исключает соседние группы", async () => {
  const exports = load();
  const commands = new Map();
  const requests = [];
  let choices;
  const noEvent = () => ({ dispose() {} });
  const vscode = {
    ...exports.vscode,
    ProgressLocation: { Notification: 1 },
    workspace: {
      isTrusted: true,
      textDocuments: [],
      registerFileSystemProvider: noEvent,
    },
    commands: {
      registerCommand: (id, run) => {
        commands.set(id, run);
        return noEvent();
      },
    },
    window: {
      withProgress: (_, run) => run({}, { onCancellationRequested: noEvent }),
      showErrorMessage: (message) => assert.fail(message),
      showInformationMessage: () => assert.fail("Отличия должны быть найдены"),
      showQuickPick: async (items) => {
        choices = items;
      },
    },
  };
  const source = new core.DatabaseSource(
    {
      request: async (method, params) => {
        requests.push({ method, params });
        return {
          objects: [
            {
              object: "Catalog.Удалённый",
              changed: [],
              baseOnly: [],
              localOnly: ["Catalogs/Удалённый.xml"],
            },
            {
              object: "Catalog.Существующий",
              changed: ["Catalogs/Существующий.xml"],
              baseOnly: [],
              localOnly: [],
            },
            {
              object: "Document.Посторонний",
              changed: [],
              baseOnly: [],
              localOnly: ["Documents/Посторонний.xml"],
            },
          ],
        };
      },
    },
    "/work",
    "База",
  );
  const module = { exports: {} };
  vm.runInNewContext(
    fs.readFileSync(
      path.join(__dirname, "../out/database-commands.js"),
      "utf8",
    ),
    {
      exports: module.exports,
      module,
      AbortController,
      require: (id) =>
        id === "vscode"
          ? vscode
          : id === "./database-files"
            ? exports
            : id === "./configuration-tree"
              ? core
              : require(id),
    },
  );
  const context = { subscriptions: [] };
  module.exports.registerDatabaseCommands(context, {
    source: () => source,
    onDidChangeSource: noEvent,
    objectNames: () =>
      assert.fail("Нельзя ограничивать группу именами из базы"),
  });
  await commands.get("ibcmd.source.compare")({
    type: "group",
    kind: "Catalog",
  });
  assert.equal(requests[0].method, "objects/status");
  assert.equal(requests[0].params.objects, undefined);
  assert.deepEqual(Array.from(choices, (i) => i.difference.path).sort(), [
    "Catalogs/Существующий.xml",
    "Catalogs/Удалённый.xml",
  ]);
  for (const subscription of context.subscriptions) subscription.dispose();
});

test("область сравнения общих и вложенных объектов не захватывает соседей", () => {
  assert.equal(
    core.groupContains({ kind: "Common" }, "CommonModule.Модуль"),
    true,
  );
  assert.equal(core.groupContains({ kind: "Common" }, "Catalog.Товары"), false);
  const group = {
    kind: "Form",
    parent: { type: "object", kind: "Catalog", fullName: "Catalog.Товары" },
  };
  assert.equal(core.groupContains(group, "Catalog.Товары.Form.Форма"), true);
  assert.equal(core.groupContains(group, "Catalog.Склады.Form.Форма"), false);
  assert.equal(
    core.groupContains(group, "Catalog.Товары.Template.Макет"),
    false,
  );
});

test("ошибки сервера не раскрывают его текст; недоверенная область не читается", async () => {
  const { DatabaseFiles, vscode } = load();
  const files = new DatabaseFiles();
  const uri = files.uri(
    {
      read: async () => {
        throw Error("секретные данные сервера");
      },
    },
    "Configuration.xml",
  );
  await assert.rejects(
    files.readFile(uri),
    (e) => e.code === "Unavailable" && !e.message.includes("секретные"),
  );
  vscode.workspace.isTrusted = false;
  await assert.rejects(files.readFile(uri), { code: "NoPermissions" });
  files.dispose();
});

test("сравнение объединяет файлы групп и различает отсутствующие стороны", () => {
  const { differences } = load();
  const row = {
    object: "Catalog.Товары",
    changed: ["Catalogs/Товары.xml"],
    baseOnly: ["Catalogs/Товары/Ext/Module.bsl"],
    localOnly: ["Catalogs/Товары/Ext/Old.bsl"],
  };
  const result = differences({ objects: [row, row] });
  assert.equal(result.length, 3);
  assert.equal(
    result.find((x) => x.path.endsWith("Old.bsl")).state,
    "localOnly",
  );
  assert.throws(() =>
    differences({ objects: [{ ...row, localOnly: ["../secret"] }] }),
  );
  assert.throws(() =>
    differences({ objects: [{ ...row, localOnly: row.changed }] }),
  );
  assert.throws(() => differences({ objects: [{ object: "Catalog.X" }] }));
});

test("reopening the same virtual file keeps its in-flight read valid", async () => {
  const { DatabaseFiles } = load();
  const files = new DatabaseFiles();
  let finish,
    calls = 0;
  const source = {
    read: async () => {
      calls++;
      return new Promise((resolve) => (finish = resolve));
    },
  };
  try {
    const first = files.uri(source, "Ext/Module.bsl");
    const a = files.readFile(first);
    const second = files.uri(source, "Ext/Module.bsl");
    const b = files.readFile(second);
    const reads = Promise.all([a, b]);
    finish(Buffer.from("same source"));
    assert.equal(first.toString(), second.toString());
    assert.equal(calls, 1);
    assert.deepEqual(
      (await reads).map((bytes) => bytes.toString()),
      ["same source", "same source"],
    );
  } finally {
    files.dispose();
  }
});

test("group comparison discovers checkout-only objects and excludes other types", async () => {
  const { DatabaseFiles, differences, vscode } = load();
  const commands = new Map(),
    requests = [];
  let choices;
  const source = new core.DatabaseSource(
    {
      request: async (method, params, signal, timeout) => {
        requests.push({ method, params, signal, timeout });
        const rows = [
          {
            object: "Catalog.Removed",
            changed: [],
            baseOnly: [],
            localOnly: ["Catalogs/Removed.xml"],
          },
          {
            object: "Document.Other",
            changed: [],
            baseOnly: [],
            localOnly: ["Documents/Other.xml"],
          },
          {
            object: "CatalogOther.Wrong",
            changed: [],
            baseOnly: [],
            localOnly: ["CatalogOthers/Wrong.xml"],
          },
        ];
        return {
          objects: rows.filter(
            (row) => !params.objects || params.objects.includes(row.object),
          ),
        };
      },
    },
    "/work",
    "demo",
  );
  const noEvent = () => ({ dispose() {} });
  vscode.workspace.textDocuments = [];
  vscode.workspace.registerFileSystemProvider = noEvent;
  vscode.commands = {
    registerCommand: (name, run) => {
      commands.set(name, run);
      return noEvent();
    },
  };
  vscode.window = {
    withProgress: (_, run) =>
      run(
        {},
        { isCancellationRequested: false, onCancellationRequested: noEvent },
      ),
    showQuickPick: async (items) => {
      choices = items;
    },
    showInformationMessage() {},
    showErrorMessage: (message) => assert.fail(message),
  };
  vscode.ProgressLocation = { Notification: 1 };
  const view = {
    source: () => source,
    onDidChangeSource: noEvent,
    objectNames: async () => ["Catalog.Existing"],
  };
  const module = { exports: {} };
  vm.runInNewContext(
    fs.readFileSync(
      path.join(__dirname, "../out/database-commands.js"),
      "utf8",
    ),
    {
      module,
      exports: module.exports,
      AbortController,
      require: (id) =>
        id === "vscode"
          ? vscode
          : id === "./configuration-tree"
            ? core
            : id === "./database-files"
              ? { DatabaseFiles, differences }
              : id === "./configuration-view"
                ? {}
                : require(id),
    },
  );
  const provider = module.exports.registerDatabaseCommands(
    { subscriptions: [] },
    view,
  );
  try {
    const group = {
      id: "Configuration/Catalogs",
      kind: "Catalog",
      type: "group",
      parent: { kind: "Configuration", fullName: "Configuration" },
    };
    await commands.get("ibcmd.source.compare")(group);
    assert.equal(requests.length, 1);
    assert.equal(requests[0].params.objects, undefined);
    assert.ok(
      Number.isFinite(requests[0].timeout) && requests[0].timeout >= 600000,
    );
    assert.ok(requests[0].signal instanceof AbortSignal);
    assert.deepEqual(
      Array.from(choices, (item) => item.difference.path),
      ["Catalogs/Removed.xml"],
    );
  } finally {
    provider.dispose();
  }
});

test("group scope respects root types, shared types and exact nested owners", () => {
  assert.equal(
    core.groupContains({ kind: "Common" }, "CommonModule.LocalOnly"),
    true,
  );
  assert.equal(core.groupContains({ kind: "Common" }, "Catalog.Other"), false);
  const forms = { kind: "Form", parent: { fullName: "Catalog.Products" } };
  assert.equal(
    core.groupContains(forms, "Catalog.Products.Form.LocalOnly"),
    true,
  );
  assert.equal(
    core.groupContains(forms, "Catalog.ProductsOther.Form.Other"),
    false,
  );
  assert.equal(
    core.groupContains(forms, "Catalog.Products.Template.Other"),
    false,
  );
});
