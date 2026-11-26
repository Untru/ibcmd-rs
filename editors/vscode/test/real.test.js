const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { RpcClient } = require("../out/rpc");
const { editConfig, saveConfig } = require("../out/config");
const binary = process.env.IBCMD_TEST_BINARY;

test(
  "скопированный rows-dir остаётся файловым источником настоящего сервера",
  { skip: !binary },
  async () => {
    const dir = await fs.mkdtemp(path.join(os.tmpdir(), "ibcmd-rows-edit-"));
    const rowsDir = path.join(
      os.tmpdir(),
      "ibcmd-external-editor-server-export",
      "rows/demo",
    );
    const database = { name: "Архив", platform: "8.3.27", "rows-dir": rowsDir };
    const file = path.join(dir, "ibcmd-rs.toml");
    await saveConfig(file, "", editConfig("", undefined, database));
    const rpc = new RpcClient(binary, dir);
    try {
      await rpc.request("initialize", {
        protocolVersion: "1.1",
        workspaceFolder: dir,
      });
      const listed = (await rpc.request("infobases/list")).infobases;
      const copied = listed.find((base) => base.id === "Архив");
      assert.equal(copied.source, "rows-dir");
      assert.equal(copied.server, undefined);
      assert.equal(path.resolve(copied.rowsDir), path.resolve(rowsDir));
      await rpc.request("infobases/connect", { infobase: copied.id });
      const read = await rpc.request("source/read", {
        infobase: copied.id,
        path: "Configuration.xml",
      });
      assert.equal(read.exists, true);
    } finally {
      await rpc.dispose();
    }
  },
);

test(
  "editor-created TOML is visible in real settings show and real stdio list",
  { skip: !binary },
  async () => {
    const dir = await fs.mkdtemp(path.join(os.tmpdir(), "ibcmd-real-test-"));
    const file = path.join(dir, "ibcmd-rs.toml");
    const next = editConfig("# keep this comment\n", undefined, {
      server: "local-test",
      name: "Редактор",
      platform: "8.3.27",
    });
    await saveConfig(file, "", next);
    const env = { ...process.env, IBCMD_RS_CONFIG: file };
    delete env.IBCMD_DB_PSW;
    const output = execFileSync(
      binary,
      [
        "settings",
        "show",
        "--db-server",
        "local-test",
        "--db-name",
        "Редактор",
        "--json",
      ],
      { cwd: dir, env, encoding: "utf8" },
    );
    const report = JSON.parse(output);
    assert.equal(report.databases[0].name, "Редактор");
    assert.equal(report.databases[0].matches, true);
    const rpc = new RpcClient(binary, dir);
    try {
      const init = await rpc.request("initialize", {
        protocolVersion: "1.1",
        workspaceFolder: dir,
      });
      assert.equal(init.protocolVersion, "1.4");
      const listed = await rpc.request("infobases/list");
      assert.ok(listed.infobases.some((b) => b.id === "local-test/Редактор"));
      await assert.rejects(
        rpc.request("infobases/connect", {
          infobase: "local-test/Редактор",
          auth: "sql",
        }),
        (e) => e.code === -32602,
      );
    } finally {
      await rpc.dispose();
    }
  },
);
