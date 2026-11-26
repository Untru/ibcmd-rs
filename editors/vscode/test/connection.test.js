// Execute the compiled extension state machine with deterministic synthetic dependencies.
// No SQL connection or real credentials are used by this harness.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
async function harness({
  stored,
  input = async () => "synthetic-new",
  connect = async () => ({}),
  store = async () => {},
  listed,
  text = "",
  snapshot = async () => text,
  request = async () => undefined,
  source = "mssql",
  initialize = async () => ({}),
  authentication = "windows",
} = {}) {
  const errors = [],
    infos = [];
  const commands = new Map(),
    stores = [],
    clients = [];
  const f = {
    name: "fixture",
    uri: {
      scheme: "file",
      fsPath: "/synthetic",
      toString: () => "file:///synthetic",
    },
  };
  let picked = "a",
    cancel = () => {};
  const noEvent = () => ({ dispose() {} });
  const bases =
    listed ??
    ["a", "b"].map((id) => ({
      id,
      name: id,
      source,
      platform: "8.3.27",
    }));
  const saved = [],
    updates = [],
    offered = [];
  const values = {
    executable: "synthetic",
    connections: ["a", "b"],
    connectionMetadata: {
      a: { auth: "sql", user: "fixture-user" },
      b: { auth: "windows" },
    },
  };
  const token = {
    isCancellationRequested: false,
    onCancellationRequested: (callback) => {
      cancel = () => {
        token.isCancellationRequested = true;
        callback();
      };
      return noEvent();
    },
  };
  class RpcClient {
    constructor(_, __, onCrash = () => {}) {
      this.onCrash = onCrash;
      this.disposed = false;
      clients.push(this);
    }
    async dispose() {
      this.disposed = true;
    }
    async request(method, params, signal) {
      if (method === "initialize") return initialize(this, params);
      if (method === "infobases/list") return { infobases: bases };
      if (method === "infobases/connect")
        return connect(params, signal, () => this.onCrash());
      const result = await request(method, params, signal);
      return result ?? { conclusive: true, pending: false };
    }
  }
  const vscode = {
    StatusBarAlignment: { Left: 1 },
    ConfigurationTarget: { WorkspaceFolder: 3 },
    ProgressLocation: { Notification: 1 },
    workspace: {
      isTrusted: true,
      textDocuments: [],
      workspaceFolders: [f],
      getConfiguration: () => ({
        get: (key, fallback) => values[key] ?? fallback,
        update: async (key, value) => {
          updates.push({ key, value });
          values[key] = value;
        },
      }),
      onDidChangeConfiguration: noEvent,
      onDidChangeWorkspaceFolders: noEvent,
      createFileSystemWatcher: () => ({
        onDidChange: noEvent,
        onDidCreate: noEvent,
        onDidDelete: noEvent,
        dispose() {},
      }),
    },
    commands: {
      registerCommand: (name, run) => {
        commands.set(name, run);
        return noEvent();
      },
    },
    window: {
      createOutputChannel: () => ({ appendLine() {}, show() {}, dispose() {} }),
      createStatusBarItem: () => ({ text: "", show() {}, dispose() {} }),
      showInputBox: input,
      showQuickPick: async (items) => {
        items = await items;
        offered.push(
          items.filter((item) => item.base).map((item) => item.base.id),
        );
        return (
          items.find((item) => item.base?.id === picked) ??
          items.find((item) => item.value === authentication)
        );
      },
      showErrorMessage(message) {
        errors.push(message);
      },
      showInformationMessage(message) {
        infos.push(message);
      },
      showWarningMessage: async (_, options, label) => label,
      withProgress: (_, task) => task({}, token),
    },
  };
  const module = { exports: {} };
  const extensionSource = fs.readFileSync(
    path.join(__dirname, "../out/extension.js"),
    "utf8",
  );
  vm.runInNewContext(extensionSource, {
    exports: module.exports,
    module,
    AbortController,
    require: (id) => {
      if (id === "vscode") return vscode;
      if (id === "./rpc")
        return { RpcClient, RpcError: class extends Error {} };
      if (id === "./config")
        return {
          ...require("../out/config"),
          snapshot,
          saveConfig: async (_, expected, next) => {
            assert.equal(expected, text);
            saved.push(next);
          },
        };
      // Object rendering is covered separately; isolate the connection state machine.
      if (id === "./object-view")
        return {
          ObjectView: class {
            clear() {}
          },
        };
      if (id === "./executable") return require("../out/executable");

      if (id === "./status") return require("../out/status");
      if (id === "./apply") return require("../out/apply");
      if (id === "./import") return require("../out/import");
      if (id === "./configuration-view")
        return {
          ConfigurationView: class {
            connect() {}
            disconnect() {}
          },
        };
      if (id === "./database-commands")
        return { registerDatabaseCommands() {} };
      return require(id);
    },
  });
  const api = await module.exports.activate({
    subscriptions: [],
    extensionPath: path.resolve(__dirname, ".."),
    secrets: {
      get: async () => stored,
      delete: async () => {},
      store: async (key, value) => {
        stores.push({ key, value });
        await store();
      },
    },
    workspaceState: { update: async () => {} },
  });
  return {
    stores,
    clients,
    values,
    saved,
    updates,
    errors,
    offered,
    infos,
    invoke: (name) => commands.get(name)(),
    edit: (id) => {
      picked = id;
      return commands.get("ibcmd.edit")();
    },
    status: api.status,
    cancel: () => cancel(),
    select: (id = "a") => {
      picked = id;
      return commands.get("ibcmd.select")();
    },
  };
}

test("редактирование файлового источника сохраняет rows-dir без запросов SQL", async () => {
  const { readConfig, idOf } = require("../out/config");
  for (const server of [undefined, "сервер"]) {
    for (const local of [false, true]) {
      const prompts = [];
      const initialized = [];
      const entry = {
        name: "Архив",
        server,
        source: "rows-dir",
        rowsDir: path.resolve("данные", "Архив"),
        platform: "8.3.27",
        mask: false,
      };
      entry.id = idOf(entry);
      const text = local
        ? `[[database]]\nname="Архив"\nplatform="8.3.27"\nrows-dir="данные/Архив"\n${server ? 'server="сервер"\n' : ""}`
        : "";
      const h = await harness({
        listed: [entry],
        text,
        initialize: async (_, params) => {
          initialized.push(params);
          return {};
        },
        input: async ({ prompt, value }) => {
          prompts.push(prompt);
          return value;
        },
      });
      await h.edit(entry.id);
      assert.deepEqual(h.errors, []);
      const [saved] = readConfig(h.saved[0]);
      assert.equal(saved["rows-dir"], local ? "данные/Архив" : entry.rowsDir);
      assert.equal(saved.server, server);
      assert.equal(saved.name, entry.name);
      assert.equal(h.stores.length, 0);
      assert.ok(prompts.every((p) => !p.includes("SQL")));
      const validation = initialized.find((p) => p.infobases);
      assert.equal(
        validation.infobases[0].rowsDir,
        local ? undefined : entry.rowsDir,
      );
      assert.equal(h.values.connectionMetadata[idOf(saved)].auth, undefined);
    }
  }
});
test("new password is persisted only after a successful current connect", async () => {
  const started = deferred(),
    finish = deferred();
  const h = await harness({
    connect: async () => {
      started.resolve();
      return finish.promise;
    },
  });
  const selecting = h.select();
  await started.promise;
  assert.equal(h.stores.length, 0);
  finish.resolve({});
  await selecting;
  assert.equal(h.stores.length, 1);
  assert.equal(h.stores[0].value, "synthetic-new");
});
test("failed login and cancelled prompt never store a new password", async () => {
  for (const options of [
    {
      connect: async () => {
        throw new Error("synthetic rejection");
      },
    },
    { input: async () => undefined },
  ]) {
    const h = await harness(options);
    await h.select();
    assert.equal(h.stores.length, 0);
  }
});
test("stale prompted password is not stored after a newer selection", async () => {
  const started = deferred(),
    finish = deferred();
  const h = await harness({
    input: () => {
      started.resolve();
      return finish.promise;
    },
  });
  const selecting = h.select();
  await started.promise;
  await h.select("b");
  finish.resolve("synthetic-stale");
  await selecting;
  assert.equal(h.stores.length, 0);
  assert.ok(h.status().includes(" b ·"));
});
test("stale or cancelled connect completion never commits a password", async () => {
  for (const stale of [true, false]) {
    const started = deferred(),
      finish = deferred();
    const h = await harness({
      connect: (params) => {
        if (params.infobase === "b") return {};
        started.resolve();
        return finish.promise;
      },
    });
    const selecting = h.select();
    await started.promise;
    if (stale) await h.select("b");
    else h.cancel();
    finish.resolve({});
    await selecting;
    assert.equal(h.stores.length, 0);
    if (!stale) {
      assert.ok(h.status().includes("disconnected"));
      assert.ok(h.clients.every((client) => client.disposed));
    }
  }
});
test("cancellation during non-cancellable secret storage still cleans up the session", async () => {
  const started = deferred(),
    finish = deferred();
  const h = await harness({
    store: () => {
      started.resolve();
      return finish.promise;
    },
  });
  const selecting = h.select();
  await started.promise;
  h.cancel();
  finish.resolve();
  await selecting;
  assert.ok(h.status().includes("disconnected"));
  assert.ok(h.clients.every((client) => client.disposed));
});
test("selection change during non-cancellable storage cannot restore stale status", async () => {
  const started = deferred(),
    finish = deferred();
  const h = await harness({
    store: () => {
      started.resolve();
      return finish.promise;
    },
  });
  const selecting = h.select();
  await started.promise;
  await h.select("b");
  finish.resolve();
  await selecting;
  assert.ok(h.status().includes(" b ·"));
});
test("a stored password is reused without prompting or rewriting on failure", async () => {
  let prompts = 0;
  const passwords = [];
  const h = await harness({
    stored: "synthetic-existing",
    input: async () => {
      prompts++;
      return undefined;
    },
    connect: (params) => {
      passwords.push(params.password);
      throw new Error("synthetic rejection");
    },
  });
  await h.select();
  assert.equal(prompts, 0);
  assert.deepEqual(passwords, ["synthetic-existing"]);
  assert.equal(h.stores.length, 0);
});

test("import-capable client advertises 1.4 and cannot connect to a 1.1-only server", async () => {
  const versions = [];
  let connections = 0;
  const h = await harness({
    initialize: async (_client, params) => {
      versions.push(params.protocolVersion);
      if (params.protocolVersion !== "1.1")
        throw new Error("unsupported protocol version");
      return {};
    },
    connect: async () => {
      connections++;
      return {};
    },
  });
  await h.select();
  assert.deepEqual(versions, ["1.4"]);
  assert.equal(connections, 0);
  assert.equal(h.stores.length, 0);
});

test("Edit preserves an omitted inherited server and excludes file databases", async () => {
  const text =
    'db-server="effective-host"\n[[database]]\nname="a"\nplatform="8.3.27"\n';
  const h = await harness({
    text,
    listed: [
      { id: "a", name: "a", platform: "8.3.27", source: "mssql" },
      { id: "file", name: "file", source: "file-db" },
      { id: "unknown", name: "unknown", source: "future-source" },
    ],
    input: async (options) => {
      if (options.prompt.startsWith("SQL Server")) {
        assert.equal(options.value, "");
        assert.equal(options.validateInput(""), undefined);
      }
      return options.value;
    },
  });
  await h.edit("a");
  assert.deepEqual(h.errors, []);
  assert.equal(h.saved.length, 1);
  assert.equal(
    require("../out/config").readConfig(h.saved[0])[0].server,
    undefined,
  );
  assert.deepEqual(Array.from(h.offered[0]), ["a"]);
});
test("Edit copies inherited offline rows with its source intact", async () => {
  const h = await harness({
    listed: [
      {
        id: "a",
        name: "a",
        platform: "8.3.27",
        source: "rows-dir",
        rowsDir: "/synthetic/original/rows",
      },
    ],
    input: async (options) => options.value,
  });
  await h.edit("a");
  assert.deepEqual(h.errors, []);
  assert.equal(h.saved.length, 1);
  assert.equal(
    require("../out/config").readConfig(h.saved[0])[0]["rows-dir"],
    "/synthetic/original/rows",
  );
});

test("Edit refuses inherited offline entries without a source directory", async () => {
  const h = await harness({
    listed: [{ id: "a", name: "a", platform: "8.3.27", source: "rows-dir" }],
    input: async (options) => options.value,
  });
  await h.edit("a");
  assert.equal(h.saved.length, 0);
  assert.equal(h.errors.length, 1);
});

test("current startup and connect crashes report one sanitized error", async () => {
  for (const startup of [true, false]) {
    let initialized = 0;
    const h = await harness({
      initialize: async (client) => {
        if (startup && ++initialized === 2) {
          client.onCrash();
          throw new Error("ENOENT synthetic-private-path");
        }
        return {};
      },
      connect: async (_, __, crash) => {
        crash();
        throw new Error("synthetic-private-server-exit");
      },
    });
    await h.select();
    assert.equal(h.errors.length, 1);
    assert.ok(!h.errors[0].includes("synthetic-private"));
    assert.ok(h.status().includes("disconnected"));
    assert.equal(h.stores.length, 0);
  }
});
test("a late crash from an old connection cannot notify or replace the new selection", async () => {
  const started = deferred(),
    finish = deferred();
  const h = await harness({
    connect: async (params, _, crash) => {
      if (params.infobase === "a") {
        started.resolve();
        await finish.promise;
        crash();
        throw new Error("synthetic old crash");
      }
      return {};
    },
  });
  const selecting = h.select();
  await started.promise;
  await h.select("b");
  finish.resolve();
  await selecting;
  assert.equal(h.errors.length, 0);
  assert.ok(h.status().includes(" b ·"));
  assert.equal(h.stores.length, 0);
});

test("apply command refuses a connection changed during settings snapshot", async () => {
  let gate,
    entered,
    waitOnce = false;
  const methods = [];
  const h = await harness({
    stored: "cached",
    snapshot: async () => {
      if (waitOnce) {
        waitOnce = false;
        entered.resolve();
        await gate.promise;
      }
      return "";
    },
    request: async (method) => {
      methods.push(method);
      return undefined;
    },
  });
  await h.select("a");
  methods.length = 0;
  gate = deferred();
  entered = deferred();
  waitOnce = true;
  const applying = h.invoke("ibcmd.apply");
  await entered.promise;
  await h.select("b");
  gate.resolve();
  await applying;
  assert.ok(!methods.includes("config/apply"));
  assert.ok(!methods.includes("config/apply/prepare"));
});

test("pending snapshot failure after committed import preserves success", async () => {
  let published = false;
  const h = await harness({
    stored: "cached",
    source: "file-db",
    snapshot: async () => {
      if (published) throw new Error("unreadable settings");
      return "";
    },
    request: async (method) => {
      if (method === "objects/import/prepare")
        return {
          plan: "confirmed",
          preflight: {
            ready: true,
            files: ["CommonModules/Example/Ext/Module.bsl"],
            missingReferences: [],
          },
        };
      if (method === "objects/import") {
        published = true;
        return { report: {} };
      }
    },
  });
  await h.select("b");
  // runImport expects its own confirmation label.
  await h.invoke("ibcmd.importChanged");
  assert.equal(published, true);
  assert.deepEqual(h.errors, []);
  assert.ok(h.infos.some((message) => message.includes("CONFIGSAVE")));
});

test("Edit preserves an omitted inherited server and excludes file databases", async () => {
  const text =
    'db-server="effective-host"\n[[database]]\nname="a"\nplatform="8.3.27"\n';
  const h = await harness({
    text,
    listed: [
      { id: "a", name: "a", platform: "8.3.27", source: "mssql" },
      { id: "file", name: "file", source: "file-db" },
      { id: "unknown", name: "unknown", source: "future-source" },
    ],
    input: async (options) => {
      if (options.prompt.startsWith("SQL Server")) {
        assert.equal(options.value, "");
        assert.equal(options.validateInput(""), undefined);
      }
      return options.value;
    },
  });
  await h.edit("a");
  assert.deepEqual(h.errors, []);
  assert.equal(h.saved.length, 1);
  assert.equal(
    require("../out/config").readConfig(h.saved[0])[0].server,
    undefined,
  );
  assert.deepEqual(Array.from(h.offered[0]), ["a"]);
});
test("Edit copies inherited offline rows with its source intact", async () => {
  const h = await harness({
    listed: [
      {
        id: "a",
        name: "a",
        platform: "8.3.27",
        source: "rows-dir",
        rowsDir: "/synthetic/original/rows",
      },
    ],
    input: async (options) => options.value,
  });
  await h.edit("a");
  assert.deepEqual(h.errors, []);
  assert.equal(h.saved.length, 1);
  assert.equal(
    require("../out/config").readConfig(h.saved[0])[0]["rows-dir"],
    "/synthetic/original/rows",
  );
});

test("Edit refuses inherited offline entries without a source directory", async () => {
  const h = await harness({
    listed: [{ id: "a", name: "a", platform: "8.3.27", source: "rows-dir" }],
    input: async (options) => options.value,
  });
  await h.edit("a");
  assert.equal(h.saved.length, 0);
  assert.equal(h.errors.length, 1);
});

test("current startup and connect crashes report one sanitized error", async () => {
  for (const startup of [true, false]) {
    let initialized = 0;
    const h = await harness({
      initialize: async (client) => {
        if (startup && ++initialized === 2) {
          client.onCrash();
          throw new Error("ENOENT synthetic-private-path");
        }
        return {};
      },
      connect: async (_, __, crash) => {
        crash();
        throw new Error("synthetic-private-server-exit");
      },
    });
    await h.select();
    assert.equal(h.errors.length, 1);
    assert.ok(!h.errors[0].includes("synthetic-private"));
    assert.ok(h.status().includes("disconnected"));
    assert.equal(h.stores.length, 0);
  }
});
test("a late crash from an old connection cannot notify or replace the new selection", async () => {
  const started = deferred(),
    finish = deferred();
  const h = await harness({
    connect: async (params, _, crash) => {
      if (params.infobase === "a") {
        started.resolve();
        await finish.promise;
        crash();
        throw new Error("synthetic old crash");
      }
      return {};
    },
  });
  const selecting = h.select();
  await started.promise;
  await h.select("b");
  finish.resolve();
  await selecting;
  assert.equal(h.errors.length, 0);
  assert.ok(h.status().includes(" b ·"));
  assert.equal(h.stores.length, 0);
});

test("apply-capable client rejects protocol 1.1 before connecting", async () => {
  const versions = [];
  const h = await harness({
    initialize: async (_, params) => {
      versions.push(params.protocolVersion);
      if (params.protocolVersion !== "1.1")
        throw new Error("unsupported protocol");
      return {};
    },
  });
  await h.select();
  assert.deepEqual(versions, ["1.4"]);
  assert.equal(h.errors.length, 1);
  assert.equal(h.stores.length, 0);
});

test("Edit validator rejects old protocol after listing without publishing settings or secrets", async () => {
  const versions = [];
  const h = await harness({
    authentication: "sql",
    input: async (options) =>
      options.password ? "synthetic-new" : options.value,
    initialize: async (_, params) => {
      versions.push(params.protocolVersion);
      if (params.infobases) throw new Error("unsupported protocol");
      return {};
    },
  });
  await h.edit("a");
  assert.deepEqual(versions, ["1.4", "1.4"]);
  assert.equal(h.errors.length, 1);
  assert.equal(h.saved.length, 0);
  assert.equal(h.updates.length, 0);
  assert.equal(h.stores.length, 0);
  assert.ok(h.clients.every((client) => client.disposed));
});
