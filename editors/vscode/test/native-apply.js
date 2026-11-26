// Opt-in acceptance on a fresh database created solely from public tiny-source.
// No fixture teardown: every database, backup and native scratch remains local.
const fs = require("node:fs/promises"),
  path = require("node:path"),
  assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const { RpcClient } = require("../out/rpc"),
  { runImport } = require("../out/import"),
  { runApply, nativeApply } = require("../out/apply");
(async () => {
  const binary = process.env.IBCMD_TEST_BINARY,
    native = process.env.IBCMD_TEST_NATIVE,
    parent = process.env.IBCMD_TEST_ROOT;
  assert.ok(
    binary && native && parent && path.isAbsolute(parent),
    "Set absolute IBCMD_TEST_ROOT plus IBCMD_TEST_BINARY and IBCMD_TEST_NATIVE",
  );
  await fs.mkdir(parent, { recursive: true });
  const root = await fs.mkdtemp(path.join(parent, "native-apply-"));
  const db = path.join(root, "disposable"),
    baseline = path.join(root, "baseline"),
    edited = path.join(root, "edited"),
    actual = path.join(root, "actual");
  const env = { ...process.env, IBCMD_TEST_KEEP_FIXTURES: "1" };
  delete env.IBCMD_DB_PSW;
  delete env.IBCMD_RS_CONFIG;
  const run = (args) =>
    execFileSync(
      native,
      args.concat([
        `--data=${path.join(root, "native-data")}`,
        `--temp=${path.join(root, "native-temp")}`,
      ]),
      { env, stdio: "ignore", timeout: 120000 },
    );
  const source = path.resolve(
    __dirname,
    "../../../tests/fixtures/file-infobase/tiny-source",
  );
  run([
    "infobase",
    "create",
    `--db-path=${db}`,
    "--create-database",
    "--locale=en_US_POSIX",
    `--import=${source}`,
    "--apply",
    "--force",
  ]);
  run(["infobase", "config", "export", `--db-path=${db}`, baseline]);
  await fs.cp(baseline, edited, { recursive: true });
  const module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
  await fs.appendFile(
    path.join(edited, module),
    "\r\n// issue 384 synthetic acceptance\r\n",
  );
  await fs.writeFile(
    path.join(root, "ibcmd-rs.toml"),
    `[[database]]\nname = "apply384"\nplatform = "8.5.1"\ndb-path = ${JSON.stringify(db)}\n`,
  );
  process.env.IBCMD_TEST_KEEP_FIXTURES = "1";
  const rpc = new RpcClient(binary, root),
    logs = [];
  const request = rpc.request.bind(rpc);
  rpc.request = async (method, ...args) => {
    try {
      return await request(method, ...args);
    } catch (error) {
      console.error(
        `Acceptance stage failed: ${method} (${error.code ?? "transport"})`,
      );
      throw error;
    }
  };
  try {
    await rpc.request("initialize", {
      protocolVersion: "1.3",
      workspaceFolder: root,
    });
    await rpc.request("infobases/connect", { infobase: "apply384" });
    assert.equal(
      (await rpc.request("config/pending", { infobase: "apply384" })).pending,
      false,
    );
    await runImport(
      rpc,
      { infobase: "apply384", sourceDir: edited, files: [module] },
      { current: async () => true, confirm: async () => true },
    );
    assert.equal(
      (await rpc.request("config/pending", { infobase: "apply384" })).pending,
      true,
    );
    const before = await fs.readFile(path.join(db, "1Cv8.1CD"));
    assert.equal(
      await runApply(rpc, "apply384", path.join(root, "recovery"), {
        current: async () => true,
        confirm: async () => false,
        log: (m) => logs.push(m),
        native: async () => {
          throw new Error("declined apply must not execute");
        },
      }),
      false,
    );
    assert.deepEqual(await fs.readFile(path.join(db, "1Cv8.1CD")), before);
    assert.equal(
      await runApply(rpc, "apply384", path.join(root, "recovery"), {
        current: async () => true,
        confirm: async () => true,
        log: (m) => logs.push(m),
        native: (plan) =>
          nativeApply(native, plan, path.join(root, "recovery")),
      }),
      true,
    );
    assert.equal(
      (await rpc.request("config/pending", { infobase: "apply384" })).pending,
      false,
    );
  } finally {
    await rpc.dispose();
  }
  run(["infobase", "config", "export", `--db-path=${db}`, actual]);
  async function files(dir, prefix = "") {
    const result = new Map();
    for (const e of await fs.readdir(dir, { withFileTypes: true })) {
      const rel = prefix + e.name;
      if (e.isDirectory())
        for (const [k, v] of await files(path.join(dir, e.name), rel + "/"))
          result.set(k, v);
      else if (rel !== "ConfigDumpInfo.xml")
        result.set(rel, await fs.readFile(path.join(dir, e.name)));
    }
    return new Map([...result].sort(([a], [b]) => a.localeCompare(b)));
  }
  assert.deepEqual(await files(actual), await files(edited));
  const inventory = async (dir) =>
    [
      ...(
        await fs.readFile(path.join(dir, "ConfigDumpInfo.xml"), "utf8")
      ).matchAll(
        /<Metadata name="([^"]+)" id="([^"]+)" configVersion="([^"]+)"/g,
      ),
    ].map((m) => ({ name: m[1], id: m[2], version: m[3] }));
  const a = await inventory(baseline),
    b = await inventory(actual);
  assert.deepEqual(
    a.map(({ name, id }) => ({ name, id })),
    b.map(({ name, id }) => ({ name, id })),
  );
  assert.notEqual(
    a.find((e) => e.name.endsWith(".Module")).version,
    b.find((e) => e.name.endsWith(".Module")).version,
  );
  const result = {
    root,
    platform: native,
    pendingBefore: false,
    pendingAfterImport: true,
    declinedApplyBytesUnchanged: true,
    pendingAfterApply: false,
    treeEqualExcludingVersionManifest: true,
    inventoryNamesAndIdsEqual: true,
    editedModuleVersionChanged: true,
    files: (await files(actual)).size,
    logs,
  };
  await fs.writeFile(
    path.join(root, "result.json"),
    JSON.stringify(result, null, 2),
  );
  console.log(JSON.stringify(result));
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
