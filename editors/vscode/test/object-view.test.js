// Real compiled view/transport; framed synthetic server and virtual time, no SQL or disk writes.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs"),
  path = require("node:path"),
  vm = require("node:vm");
const { EventEmitter } = require("node:events");
const turn = () => new Promise(setImmediate);
function harness({
  platform = "linux",
  dirty = [],
  seconds,
  delay = 40000,
  comparison = "bytes",
  baseline = true,
} = {}) {
  const paths = platform === "win32" ? path.win32 : path.posix;
  let now = 0,
    next = 0;
  const timers = new Map(),
    sent = [],
    listeners = new Set();
  const schedule = (fn, ms) => {
    const id = ++next;
    timers.set(id, { at: now + ms, fn });
    return id;
  };
  const clock = {
    async advance(ms) {
      await turn();
      const end = now + ms;
      for (;;) {
        const first = [...timers]
          .filter(([, t]) => t.at <= end)
          .sort((a, b) => a[1].at - b[1].at)[0];
        if (!first) break;
        now = first[1].at;
        timers.delete(first[0]);
        first[1].fn();
        await turn();
      }
      now = end;
      await turn();
    },
  };
  const child = new EventEmitter();
  child.stdout = new EventEmitter();
  child.stderr = { resume() {} };
  child.stdin = new EventEmitter();
  child.stdin.end = () => {};
  child.exitCode = null;
  child.killed = false;
  child.kill = () => {
    child.killed = true;
    child.exitCode = 0;
    child.emit("exit", 0);
  };
  child.stdin.write = (frame) => {
    const message = JSON.parse(frame.subarray(frame.indexOf("\r\n\r\n") + 4));
    sent.push(message);
    if (message.method === "$/cancelRequest" || message.method === "hang")
      return;
    let result = {};
    if (message.method === "objects/status")
      result = {
        objects: [
          {
            object: "CommonModule.Test",
            version: "v",
            status: "same",
            changed: [],
            baseOnly: [],
            localOnly: [],
          },
        ],
      };
    if (message.method === "tree/children")
      result = {
        nodes: [
          {
            fullName: "CommonModule.Test",
            path: "CommonModules/Test.xml",
            hasChildren: false,
          },
        ],
      };
    schedule(
      () => {
        const body = Buffer.from(
          JSON.stringify({ jsonrpc: "2.0", id: message.id, result }),
        );
        child.stdout.emit(
          "data",
          Buffer.concat([
            Buffer.from(`Content-Length: ${body.length}\r\n\r\n`),
            body,
          ]),
        );
      },
      message.method === "objects/status" ? delay : 0,
    );
  };
  const uri = {
    file(p) {
      const fsPath = paths
        .normalize(p)
        .replace(/^[A-Z]:/, (s) => s.toLowerCase());
      return { scheme: "file", fsPath, toString: () => "file:" + fsPath };
    },
    from(value) {
      return { ...value, toString: () => JSON.stringify(value) };
    },
  };
  const values = {
    sourceDirectory: platform === "win32" ? "C:\\EXPORT" : "/export",
    statusBaselineDirectory: baseline
      ? platform === "win32"
        ? "C:\\baseline"
        : "/baseline"
      : "",
    statusComparison: comparison,
    ...(seconds === undefined ? {} : { statusTimeoutSeconds: seconds }),
  };
  const vscode = {
    Uri: uri,
    EventEmitter: class {
      event() {}
      fire() {}
      dispose() {}
    },
    ThemeColor: class {},
    TreeItem: class {},
    ProgressLocation: { Notification: 1 },
    workspace: {
      textDocuments: dirty.map((p) => ({ isDirty: true, uri: uri.file(p) })),
      getConfiguration: () => ({
        get: (key, fallback) => values[key] ?? fallback,
      }),
    },
    window: {
      createTreeView: () => ({ dispose() {} }),
      registerFileDecorationProvider: () => ({ dispose() {} }),
      withProgress: (_, run) =>
        run(
          {},
          {
            isCancellationRequested: false,
            onCancellationRequested(fn) {
              listeners.add(fn);
              return { dispose: () => listeners.delete(fn) };
            },
          },
        ),
    },
  };
  const fakeFs = {
    readFile: async () => Buffer.from("<ConfigDumpInfo/>"),
    lstat: async () => ({
      isDirectory: () => true,
      isFile: () => true,
      isSymbolicLink: () => false,
    }),
  };
  const modules = new Map();
  function load(name) {
    if (modules.has(name)) return modules.get(name);
    const module = { exports: {} };
    modules.set(name, module.exports);
    vm.runInNewContext(
      fs.readFileSync(path.join(__dirname, "../out", name + ".js"), "utf8"),
      {
        module,
        exports: module.exports,
        Buffer,
        AbortController,
        process: { platform, env: {} },
        setTimeout: schedule,
        clearTimeout: (id) => timers.delete(id),
        require(id) {
          if (id === "vscode") return vscode;
          if (id === "node:path") return paths;
          if (id === "node:fs/promises") return fakeFs;
          if (id === "node:child_process") return { spawn: () => child };
          if (id === "./git-baseline")
            return {
              gitBaseline() {
                throw new Error("Unexpected Git baseline");
              },
            };
          if (id.startsWith("./")) return load(id.slice(2));
          return require(id);
        },
      },
    );
    return module.exports;
  }
  const rpc = new (load("rpc").RpcClient)("synthetic", "/synthetic");
  const view = new (load("object-view").ObjectView)({ subscriptions: [] });
  return {
    view,
    rpc,
    clock,
    sent,
    uri,
    cancel: () => [...listeners].forEach((fn) => fn()),
    folder: {
      uri: uri.file(platform === "win32" ? "c:\\workspace" : "/workspace"),
    },
    close: () => {
      child.kill();
      view.dispose();
    },
  };
}
test("Windows dirty source casing blocks status RPC", async () => {
  const h = harness({
    platform: "win32",
    dirty: ["c:\\export\\CommonModules\\Test.xml"],
    delay: 1000,
  });
  try {
    const p = h.view.refresh(h.rpc, h.folder, "demo");
    await h.clock.advance(5000);
    await p;
    assert.equal(h.sent.filter((m) => m.method === "objects/status").length, 0);
    assert.equal(h.view.snapshot().length, 0);
    assert.equal(h.view.getChildren()[0].state, "unknown");
  } finally {
    h.close();
  }
});
test("Windows badges ignore casing but preserve folder and drive boundaries", async () => {
  const h = harness({
    platform: "win32",
    dirty: ["C:\\EXPORT-other\\file.xml", "D:\\EXPORT\\file.xml"],
    delay: 1000,
  });
  try {
    const p = h.view.refresh(h.rpc, h.folder, "demo");
    await h.clock.advance(5000);
    await p;
    assert.equal(h.view.snapshot()[0].state, "same");
    for (const p of [
      "c:\\export\\commonmodules\\test.xml",
      "c:\\export\\commonmodules\\test",
      "c:\\export\\commonmodules\\test\\Ext\\Module.bsl",
    ])
      assert.equal(h.view.provideFileDecoration(h.uri.file(p)).badge, "=");
    for (const p of [
      "c:\\export\\CommonModules\\Test-other\\file",
      "d:\\export\\CommonModules\\Test.xml",
    ])
      assert.equal(h.view.provideFileDecoration(h.uri.file(p)), undefined);
  } finally {
    h.close();
  }
});
test("POSIX path casing stays distinct", async () => {
  const h = harness({ dirty: ["/EXPORT/CommonModules/Test.xml"], delay: 1000 });
  try {
    const p = h.view.refresh(h.rpc, h.folder, "demo");
    await h.clock.advance(5000);
    await p;
    assert.equal(h.view.snapshot()[0].state, "same");
    assert.equal(
      h.view.provideFileDecoration(
        h.uri.file("/EXPORT/CommonModules/Test.xml"),
      ),
      undefined,
    );
  } finally {
    h.close();
  }
});
test("both status reads exceed 30 seconds under the bounded status budget", async () => {
  for (const seconds of [undefined, 60, 0, 3601, 1.5]) {
    const h = harness({ seconds });
    try {
      const p = h.view.refresh(h.rpc, h.folder, "demo");
      await h.clock.advance(90000);
      await p;
      assert.equal(h.view.snapshot()[0]?.state, "same", `budget ${seconds}`);
      assert.equal(
        h.sent.filter((m) => m.method === "objects/status").length,
        2,
      );
      assert.equal(
        h.sent.filter((m) => m.method === "$/cancelRequest").length,
        0,
      );
    } finally {
      h.close();
    }
  }
});
test("deadline and explicit cancel prevent late clean publication", async () => {
  for (const explicit of [false, true]) {
    const h = harness({ seconds: explicit ? 60 : 30 });
    try {
      const p = h.view.refresh(h.rpc, h.folder, "demo");
      await h.clock.advance(31000);
      if (explicit) h.cancel();
      await h.clock.advance(60000);
      await p;
      assert.equal(h.view.snapshot().length, 0);
      assert.ok(h.sent.some((m) => m.method === "$/cancelRequest"));
    } finally {
      h.close();
    }
  }
});
test("other RPC calls keep the 30-second default", async () => {
  const h = harness();
  try {
    const p = assert.rejects(h.rpc.request("hang"), /timed out/);
    await h.clock.advance(31000);
    await p;
  } finally {
    h.close();
  }
});

test("native status and tree use one view with and without baseline", async () => {
  for (const baseline of [false, true]) {
    const h = harness({ comparison: "native", baseline, delay: 0 });
    try {
      const pending = h.view.refresh(h.rpc, h.folder, "demo");
      await h.clock.advance(5000);
      await pending;
      const statuses = h.sent.filter((m) => m.method === "objects/status");
      const trees = h.sent.filter((m) => m.method === "tree/children");
      assert.equal(statuses.length, baseline ? 2 : 1);
      assert.ok(trees.length > 0);
      for (const status of statuses) {
        assert.equal(Boolean(status.params.versionBaselineDir), baseline);
      }
      // Server status selects main only when versionBaselineDir is supplied.
      for (const tree of trees)
        assert.equal(tree.params.mainConfiguration, baseline);
    } finally {
      h.close();
    }
  }
});
