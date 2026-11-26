const { test } = require("node:test");
const assert = require("node:assert/strict");
const { runApply, nativeApply } = require("../out/apply");
function harness(backend = "own") {
  const calls = [],
    logs = [];
  const rpc = {
    request: async (method, params) => {
      calls.push({ method, params });
      return { backend, modes: ["exclusive"], warning: "Close sessions" };
    },
  };
  const ui = {
    current: async () => true,
    confirm: async () => true,
    log: (m) => logs.push(m),
    native: async () => calls.push({ method: "native" }),
  };
  return { calls, logs, rpc, ui };
}
test("apply requires explicit consent; decline writes nothing", async () => {
  const h = harness();
  h.ui.confirm = async () => false;
  assert.equal(await runApply(h.rpc, "lab", "/recovery", h.ui), false);
  assert.deepEqual(
    h.calls.map((c) => c.method),
    ["config/apply/prepare"],
  );
});
test("own apply sends only exclusive with consent and recovery", async () => {
  const h = harness();
  assert.equal(await runApply(h.rpc, "lab", "/recovery", h.ui), true);
  assert.deepEqual(h.calls[1], {
    method: "config/apply",
    params: {
      infobase: "lab",
      mode: "exclusive",
      confirm: true,
      recoveryDir: "/recovery",
    },
  });
});
test("file-db native execution belongs to client", async () => {
  const h = harness("external-native");
  await runApply(h.rpc, "lab", "/recovery", h.ui);
  assert.deepEqual(
    h.calls.map((c) => c.method),
    ["config/apply/prepare", "native"],
  );
});
test("connection changes during consent block own and native apply", async () => {
  for (const backend of ["own", "external-native"]) {
    const h = harness(backend);
    let current = true;
    h.ui.current = async () => current;
    h.ui.confirm = async () => {
      current = false;
      return true;
    };
    await assert.rejects(
      runApply(h.rpc, "lab", "/recovery", h.ui),
      /Connection or settings changed/,
    );
    assert.equal(h.calls.length, 1);
  }
});
test("cancellation during consent blocks publication", async () => {
  const h = harness();
  const abort = new AbortController();
  h.ui.confirm = async () => {
    abort.abort();
    return true;
  };
  await assert.rejects(
    runApply(h.rpc, "lab", "/recovery", h.ui, abort.signal),
    /cancelled/,
  );
  assert.equal(h.calls.length, 1);
});
test("failed apply logs uncertain outcome without leaking error text", async () => {
  const h = harness("external-native");
  h.ui.native = async () => {
    throw new Error("private-password");
  };
  await assert.rejects(runApply(h.rpc, "lab", "/recovery", h.ui));
  assert.match(h.logs.at(-1), /uncertain/);
  assert.ok(!h.logs.join().includes("private-password"));
});
test("pre-cancelled native apply never spawns", async () => {
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(
    nativeApply(
      "/never-execute",
      { dbPath: "/disposable", platform: "8.5.1" },
      "/never-create",
      abort.signal,
    ),
    /cancelled/,
  );
});

function nativeHarness(
  pathApi = require("node:path"),
  platform = process.platform,
) {
  const fs = require("node:fs"),
    path = require("node:path"),
    vm = require("node:vm"),
    { EventEmitter } = require("node:events");
  const calls = [];
  const child = new EventEmitter();
  child.kill = () => {
    calls.push("kill");
    child.emit("exit", null);
  };
  const module = { exports: {} };
  vm.runInNewContext(
    fs.readFileSync(path.join(__dirname, "../out/apply.js"), "utf8"),
    {
      exports: module.exports,
      module,
      process: {
        platform,
        env: {
          IBCMD_DB_PSW: "secret",
          IBCMD_RS_CONFIG: "private",
          Ibcmd_Db_Psw: "mixed-secret",
          ibcmd_rs_config: "mixed-private",
          TMPDIR: "/external",
        },
      },
      setTimeout,
      clearTimeout,
      require: (id) => {
        if (id === "node:path") return pathApi;
        if (id === "node:child_process")
          return {
            spawn: (...args) => {
              calls.push(args);
              return child;
            },
          };
        if (id === "node:fs/promises")
          return {
            mkdir: async () => {},
            mkdtemp: async () => "/external/scratch",
          };
        if (id === "./import") return require("../out/import");
        if (id === "./rpc")
          return {
            serverEnvironment: (env) =>
              require("../out/rpc").serverEnvironment(env, platform),
          };
        return require(id);
      },
    },
  );
  return { native: module.exports.nativeApply, calls, child };
}
test("native apply binds argv and scratch, removes credentials and never uses shell", async () => {
  const h = nativeHarness();
  const running = h.native(
    "/native ibcmd",
    { dbPath: "/disposable db", platform: "8.5.1" },
    "/external",
  );
  await new Promise((resolve) => setImmediate(resolve));
  const [exe, args, options] = h.calls[0];
  assert.equal(exe, "/native ibcmd");
  assert.deepEqual(Array.from(args), [
    "infobase",
    "config",
    "apply",
    "--db-path=/disposable db",
    "--dynamic=disable",
    `--data=${require("node:path").join("/external/scratch", "data")}`,
    `--temp=${require("node:path").join("/external/scratch", "temp")}`,
  ]);
  assert.equal(options.shell, undefined);
  assert.equal(options.env.IBCMD_DB_PSW, undefined);
  assert.equal(options.env.IBCMD_RS_CONFIG, undefined);
  h.child.emit("exit", 0);
  await running;
});
test("native target is rechecked after asynchronous scratch creation", async () => {
  const h = nativeHarness();
  await assert.rejects(
    h.native(
      "/native",
      { dbPath: "/disposable", platform: "8.5.1" },
      "/external",
      undefined,
      async () => false,
    ),
    /Connection or settings changed/,
  );
  assert.equal(h.calls.length, 0);
});
test("native cancellation terminates process and reports uncertain outcome", async () => {
  const h = nativeHarness(),
    abort = new AbortController();
  const running = h.native(
    "/native",
    { dbPath: "/disposable", platform: "8.5.1" },
    "/external",
    abort.signal,
  );
  await new Promise((resolve) => setImmediate(resolve));
  abort.abort();
  await assert.rejects(running, /failed or was cancelled/);
  assert.ok(h.calls.includes("kill"));
});

test("native scratch argv follows Windows path semantics", async () => {
  const win = require("node:path").win32,
    h = nativeHarness(win);
  const running = h.native(
    "C:\\native\\ibcmd.exe",
    { dbPath: "C:\\disposable", platform: "8.5.1" },
    "C:\\external",
  );
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(
    h.calls[0][1][5],
    `--data=${win.join("/external/scratch", "data")}`,
  );
  assert.equal(
    h.calls[0][1][6],
    `--temp=${win.join("/external/scratch", "temp")}`,
  );
  h.child.emit("exit", 0);
  await running;
});

test("native apply removes Windows case variants from the child environment", async () => {
  const h = nativeHarness(require("node:path").win32, "win32");
  const running = h.native(
    "C:\\native\\ibcmd.exe",
    { dbPath: "C:\\db", platform: "8.5.1" },
    "C:\\scratch",
  );
  await new Promise((resolve) => setImmediate(resolve));
  const env = h.calls[0][2].env;
  h.child.emit("exit", 0);
  await running;
  for (const key of [
    "IBCMD_DB_PSW",
    "Ibcmd_Db_Psw",
    "IBCMD_RS_CONFIG",
    "ibcmd_rs_config",
  ])
    assert.equal(env[key], undefined, key);
  assert.equal(env.TMPDIR, "/external");
});
