const { test, mock } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const {
  editConfig,
  readConfig,
  saveConfig,
  snapshot,
} = require("../out/config");
const { RpcClient, serverEnvironment } = require("../out/rpc");
const { pendingText, Generation } = require("../out/status");
const database = {
  server: "sql\\instance",
  name: "УХ разработка",
  platform: "8.3.27",
};
test("TOML edit preserves BOM, CRLF, multiline strings, comments and unrelated keys", () => {
  const text =
    '\uFEFF# global\r\nplatform="8.3.27"\r\n[[database]] # entry\r\nname="""old\nname""" # name\r\nplatform=\'8.3.27\' # version\r\nrows-dir="rows/demo"\r\n';
  const result = editConfig(text, "old\nname", database);
  const parsed = readConfig(result);
  assert.equal(parsed[0].name, database.name);
  assert.equal(parsed[0].server, database.server);
  assert.ok(result.startsWith('\uFEFF# global\r\nplatform="8.3.27"'));
  assert.ok(result.includes(" # name"));
  assert.ok(result.includes(" # version"));
  assert.ok(result.includes('rows-dir="rows/demo"'));
});
test("add/edit escape strings and reject duplicates/wildcards/missing old IDs", () => {
  const text = editConfig("", undefined, database);
  assert.equal(readConfig(text)[0].server, database.server);
  assert.throws(() => editConfig(text, undefined, database), /already exists/);
  assert.throws(() => editConfig(text, "missing", database), /changed/);
  assert.throws(
    () => editConfig("", undefined, { ...database, name: "*" }),
    /wildcard|one database/,
  );
  const next = editConfig(text, `${database.server}/${database.name}`, {
    ...database,
    name: 'test "quoted"',
  });
  assert.equal(readConfig(next)[0].name, 'test "quoted"');
});
test("atomic save detects disk conflicts, locks and symlinks", async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "ibcmd-config-test-"));
  const file = path.join(dir, "ibcmd-rs.toml");
  await saveConfig(file, "", "# original\n");
  assert.equal(await snapshot(file), "# original\n");
  await fs.writeFile(file, "# concurrent\n");
  await assert.rejects(saveConfig(file, "# original\n", "# ours\n"), /changed/);
  assert.equal(await snapshot(file), "# concurrent\n");
  await fs.writeFile(file + ".ibcmd-lock", "");
  await assert.rejects(saveConfig(file, "# concurrent\n", ""), /lock/);
  if (process.platform !== "win32") {
    const link = path.join(dir, "link");
    await fs.symlink(file, link);
    await assert.rejects(snapshot(link), /regular/);
  }
  // Keep test evidence in the caller-selected temp directory; no destructive cleanup.
});

test("файловое подключение без сервера сохраняет и обновляет каталог", () => {
  const database = {
    name: "Архив",
    platform: "8.3.27",
    "rows-dir": "C:\\Данные\\Архив",
  };
  const text = editConfig("# Настройки\n", undefined, database);
  assert.deepEqual(readConfig(text), [database]);
  const updated = { ...database, "rows-dir": "D:\\Другой каталог" };
  assert.deepEqual(readConfig(editConfig(text, "Архив", updated)), [updated]);
  assert.throws(
    () => editConfig("", undefined, { ...database, "rows-dir": "" }),
    /required/,
  );
});
test("settings lock only translates EEXIST and preserves other filesystem errors", async () => {
  for (const code of ["EEXIST", "EACCES", "EPERM", "EROFS", "ENOENT", "EIO"]) {
    const error = Object.assign(new Error(`open failed: ${code}`), { code });
    const open = mock.method(fs, "open", async () => {
      throw error;
    });
    try {
      await assert.rejects(saveConfig("settings.toml", "", ""), (actual) =>
        code === "EEXIST"
          ? /settings lock exists/.test(actual.message)
          : actual === error,
      );
    } finally {
      open.mock.restore();
    }
  }
});
function rpc() {
  return new RpcClient(process.execPath, __dirname, () => {}, [
    path.join(__dirname, "fake-server.js"),
  ]);
}
test("framed transport counts UTF-8 bytes and suppresses server error text", async () => {
  const client = rpc();
  try {
    assert.deepEqual(await client.request("echo", { name: "База ё" }), {
      name: "База ё",
    });
    await assert.rejects(
      client.request("error"),
      (e) => e.code === -32014 && !e.message.includes("secret"),
    );
  } finally {
    await client.dispose();
  }
});
test("cancellation, timeout, late responses and crash settle requests", async () => {
  const client = rpc();
  const abort = new AbortController();
  const request = client.request("hang", {}, abort.signal);
  abort.abort();
  await assert.rejects(request, /cancelled/);
  await assert.rejects(client.request("late", {}, undefined, 20), /timed out/);
  assert.deepEqual(await client.request("echo", { ok: true }), { ok: true });
  await client.dispose();
  const crash = rpc();
  await assert.rejects(crash.request("crash"), /stopped/);
  await crash.dispose();
});
test("inconclusive/failed pending never becomes clean and old generations are rejected", () => {
  assert.equal(
    pendingText({ pending: false, conclusive: false }),
    "pending unknown",
  );
  assert.equal(pendingText({ pending: false }), "pending unknown");
  assert.equal(
    pendingText({ pending: false, conclusive: true }),
    "no pending changes",
  );
  assert.equal(
    pendingText({ pending: true, conclusive: true }),
    "pending changes",
  );
  const g = new Generation();
  const a = g.next();
  const b = g.next();
  assert.equal(g.current(a), false);
  assert.equal(g.current(b), true);
});

test("IDs match Rust trimming and appending to an inline array is refused", () => {
  const text =
    '[[database]]\nserver=" sql "\nname=" demo "\nplatform="8.3.27"\n';
  assert.equal(
    readConfig(
      editConfig(text, "sql/demo", {
        server: "sql",
        name: "demo",
        platform: "8.5.1",
      }),
    )[0].platform,
    "8.5.1",
  );
  assert.throws(
    () =>
      editConfig(text, undefined, {
        server: "sql",
        name: "demo",
        platform: "8.5.1",
      }),
    /already exists/,
  );
  assert.throws(() =>
    editConfig(
      'database = [{name="old",platform="8.3.27"}]',
      undefined,
      database,
    ),
  );
});

test("malformed error codes cannot leak server text and terminate the transport", async () => {
  const client = rpc();
  try {
    await assert.rejects(
      client.request("badError"),
      (e) => !e.message.includes("secret-marker"),
    );
    await assert.rejects(client.request("echo"), /send|unavailable|stopped/);
  } finally {
    await client.dispose();
  }
});

test("server environment removes Windows case variants without mutating the parent", () => {
  const source = {
    IBCMD_DB_PSW: "synthetic",
    Ibcmd_Db_Psw: "synthetic",
    ibcmd_rs_config: "synthetic.toml",
    IBCMD_RS_CONFIG: "synthetic.toml",
    PATH: "tools",
    OTHER: "kept",
  };
  assert.deepEqual(serverEnvironment(source, "win32"), {
    PATH: "tools",
    OTHER: "kept",
  });
  assert.equal(source.Ibcmd_Db_Psw, "synthetic");
  assert.equal(serverEnvironment(source, "darwin").Ibcmd_Db_Psw, "synthetic");
});
test("intentional disposal after an RPC failure does not report a crash", async () => {
  let crashes = 0;
  const client = new RpcClient(process.execPath, __dirname, () => crashes++, [
    path.join(__dirname, "fake-server.js"),
  ]);
  await assert.rejects(client.request("error"));
  await client.dispose();
  await client.dispose();
  assert.equal(crashes, 0);
  const crashed = new RpcClient(process.execPath, __dirname, () => crashes++, [
    path.join(__dirname, "fake-server.js"),
  ]);
  await assert.rejects(crashed.request("crash"));
  await crashed.dispose();
  assert.equal(crashes, 1);
});
test("editing preserves omitted inherited server and copying includes offline rows", () => {
  const text =
    '# inherited host\ndb-server="effective-host"\n[[database]]\nname="demo"\nplatform="8.3.27"\n';
  const result = editConfig(text, "demo", { name: "demo", platform: "8.5.1" });
  assert.equal(readConfig(result)[0].server, undefined);
  assert.ok(result.includes('db-server="effective-host"'));
  const copied = editConfig("", undefined, {
    name: "offline",
    platform: "8.3.27",
    "rows-dir": "/synthetic/inherited/rows",
  });
  assert.equal(readConfig(copied)[0]["rows-dir"], "/synthetic/inherited/rows");
});

test("SQL editor refuses file database selector changes without altering TOML", () => {
  const text =
    '[[database]]\nname="file-demo"\nplatform="8.3.27"\ndb-path="protected-filedb"\n';
  assert.throws(
    () =>
      editConfig(text, "file-demo", {
        server: "sql",
        name: "file-demo",
        platform: "8.3.27",
      }),
    /File database/,
  );
  assert.equal(readConfig(text)[0]["db-path"], "protected-filedb");
});

test("clearing an explicit server preserves its TOML comments", () => {
  const text =
    '[[database]]\nserver="explicit" # keep server explanation\nname="demo"\nplatform="8.3.27"\n';
  const edited = editConfig(text, "explicit/demo", {
    name: "demo",
    platform: "8.3.27",
  });
  assert.equal(readConfig(edited)[0].server, undefined);
  assert.ok(edited.includes("# keep server explanation"));
});

test("a missing executable rejects initialize and reports one transport crash", async () => {
  let crashes = 0;
  const client = new RpcClient(
    path.join(__dirname, "missing-ibcmd-fixture"),
    __dirname,
    () => crashes++,
    [],
  );
  await assert.rejects(client.request("initialize"));
  await client.dispose();
  assert.equal(crashes, 1);
});
