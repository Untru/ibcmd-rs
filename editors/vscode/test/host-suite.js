const vscode = require("vscode");
const fs = require("node:fs/promises");
const path = require("node:path");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
exports.run = async () => {
  const extension = vscode.extensions.getExtension(
    "ibcmd-rs.ibcmd-rs-connections",
  );
  assert.ok(extension);
  const api = await extension.activate();
  const all = await vscode.commands.getCommands();
  for (const c of [
    "add",
    "edit",
    "bind",
    "select",
    "refresh",
    "forgetPassword",
    "apply",
  ])
    assert.ok(all.includes(`ibcmd.${c}`));
  const f = vscode.workspace.workspaceFolders[0];
  const folderPick = vscode.window.showWorkspaceFolderPick;
  vscode.window.showWorkspaceFolderPick = async () => f;
  const cfg = vscode.workspace.getConfiguration("ibcmd", f.uri);
  await cfg.update(
    "executable",
    process.env.IBCMD_TEST_BINARY,
    vscode.ConfigurationTarget.Global,
  );
  // Drive the real extension's commands with deterministic input responses in a real extension host.
  // These UI responses are test fixtures; SQL login itself is deliberately not simulated as live evidence.
  const input = vscode.window.showInputBox;
  const quick = vscode.window.showQuickPick;
  const info = vscode.window.showInformationMessage;
  const error = vscode.window.showErrorMessage;
  const progress = vscode.window.withProgress;
  const warning = vscode.window.showWarningMessage;
  const errors = [];
  let values = [
    "УХ разработка",
    "sql-test",
    "editor_database",
    "8.3.27",
    "test-user",
    "test-only-secret-378",
  ];
  vscode.window.showInputBox = async () => values.shift();
  vscode.window.showQuickPick = async (items) => {
    items = await items;
    return items.find((i) => i.value === "sql") ?? items[0];
  };
  vscode.window.showInformationMessage = async () => undefined;
  vscode.window.showErrorMessage = async (message) => {
    errors.push(message);
  };
  try {
    await vscode.commands.executeCommand("ibcmd.add");
    assert.deepEqual(errors, []);
    const toml = await fs.readFile(
      path.join(f.uri.fsPath, "ibcmd-rs.toml"),
      "utf8",
    );
    const settings = await fs.readFile(
      path.join(f.uri.fsPath, ".vscode/settings.json"),
      "utf8",
    );
    assert.ok(toml.includes("editor_database"));
    assert.ok(settings.includes("УХ разработка"));
    assert.ok(settings.includes("test-user"));
    assert.ok(!toml.includes("test-only-secret-378"));
    assert.ok(!settings.includes("test-only-secret-378"));
    assert.deepEqual(
      vscode.workspace.getConfiguration("ibcmd", f.uri).get("connections"),
      ["sql-test/editor_database"],
    );
    const report = JSON.parse(
      execFileSync(
        process.env.IBCMD_TEST_BINARY,
        ["settings", "show", "--json"],
        {
          cwd: f.uri.fsPath,
          env: {
            ...process.env,
            IBCMD_RS_CONFIG: path.join(f.uri.fsPath, "ibcmd-rs.toml"),
          },
          encoding: "utf8",
        },
      ),
    );
    assert.ok(report.databases.some((b) => b.name === "editor_database"));
    values = [
      "УХ test",
      "sql-test",
      "editor_database",
      "8.5.1",
      "test-user",
      "changed-test-secret-378",
    ];
    await vscode.commands.executeCommand("ibcmd.edit");
    assert.deepEqual(errors, []);
    const edited = await fs.readFile(
      path.join(f.uri.fsPath, "ibcmd-rs.toml"),
      "utf8",
    );
    assert.ok(edited.includes("8.5.1"));
    assert.ok(!edited.includes("changed-test-secret-378"));
    assert.equal((edited.match(/\[\[database\]\]/g) || []).length, 1);
    // Real server parsing plus picker contract: SQL Edit cannot mutate a file-db entry.
    const fileDb = path.join(f.uri.fsPath, "untouched-file-db");
    await fs.mkdir(fileDb);
    const mixed =
      edited +
      '\n[[database]]\nname="file-fixture"\nplatform="8.3.27"\ndb-path=' +
      JSON.stringify(fileDb) +
      "\n";
    const configFile = path.join(f.uri.fsPath, "ibcmd-rs.toml");
    await fs.writeFile(configFile, mixed);
    let editable = [];
    vscode.window.showQuickPick = async (items) => {
      editable = await items;
      return undefined;
    };
    await vscode.commands.executeCommand("ibcmd.edit");
    assert.deepEqual(errors, []);
    assert.ok(
      editable.some((item) => item.base?.id === "sql-test/editor_database"),
    );
    assert.ok(!editable.some((item) => item.base?.source === "file-db"));
    assert.equal(await fs.readFile(configFile, "utf8"), mixed);
    const mixedReport = JSON.parse(
      execFileSync(
        process.env.IBCMD_TEST_BINARY,
        ["settings", "show", "--json"],
        {
          cwd: f.uri.fsPath,
          env: { ...process.env, IBCMD_RS_CONFIG: configFile },
          encoding: "utf8",
        },
      ),
    );
    assert.ok(
      mixedReport.databases.some((base) => base.name === "file-fixture"),
    );
    await fs.writeFile(
      path.join(f.uri.fsPath, "file-db-edit-evidence.toml"),
      mixed,
    );
    await fs.writeFile(configFile, edited);
    const rows = process.env.IBCMD_TEST_ROWS;
    if (!rows)
      throw new Error(
        "Set IBCMD_TEST_ROWS to committed-fixture rows generated by cargo test --test editor_server",
      );
    const offline =
      edited.replace("8.5.1", "8.3.27") +
      "\nrows-dir = " +
      JSON.stringify(rows) +
      "\n";
    await fs.writeFile(path.join(f.uri.fsPath, "ibcmd-rs.toml"), offline);
    await new Promise((resolve) => setTimeout(resolve, 100));
    vscode.window.showQuickPick = async (items) =>
      (await items).find((i) => i.base?.id === "sql-test/editor_database");
    await vscode.commands.executeCommand("ibcmd.select");
    assert.deepEqual(errors, []);
    assert.ok(api.status().includes("pending unknown"), api.status());
    assert.ok(!api.status().includes("disconnected"), api.status());
    await cfg.update(
      "statusComparison",
      "bytes",
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    // Actual RPC/rows-dir integration, separately from synthetic direction/lifecycle tests.
    const realSource = path.join(f.uri.fsPath, "real-source");
    const realBaseline = path.join(f.uri.fsPath, "real-baseline");
    await fs.mkdir(realSource);
    const cli = (...args) =>
      JSON.parse(
        execFileSync(process.env.IBCMD_TEST_BINARY, args, {
          cwd: f.uri.fsPath,
          encoding: "utf8",
        }),
      );
    const inventory = cli(
      "objects",
      "status",
      "--platform",
      "8.3.27",
      "--rows-dir",
      rows,
      "--source-dir",
      realSource,
    ).objects;
    const args = [
      "objects",
      "export",
      "--platform",
      "8.3.27",
      "--rows-dir",
      rows,
      "--target-dir",
      realSource,
    ];
    for (const object of inventory) args.push("--object", object.object);
    cli(...args);
    await fs.cp(realSource, realBaseline, { recursive: true });
    await cfg.update(
      "sourceDirectory",
      realSource,
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    await cfg.update(
      "statusBaselineDirectory",
      realBaseline,
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    await vscode.commands.executeCommand("ibcmd.select");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.equal(api.objects().length, inventory.length);
    assert.ok(
      api.objects().every((r) => r.state === "same"),
      JSON.stringify(api.objects()),
    );
    const missing = path.join(f.uri.fsPath, "missing-export");
    await fs.mkdir(missing);
    const fileList = cli(
      "objects",
      "status",
      "--platform",
      "8.3.27",
      "--rows-dir",
      rows,
      "--source-dir",
      missing,
    ).objects;
    const module = fileList
      .flatMap((r) => r.baseOnly)
      .find((p) => p.endsWith(".bsl"));
    assert.ok(module, "Committed demo fixture has a module");
    await fs.appendFile(
      path.join(realSource, module),
      "\n// local #380 test\n",
    );
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.ok(
      api.objects().some((r) => r.state === "file"),
      JSON.stringify(api.objects()),
    );
    assert.equal(
      api.objectDecoration(vscode.Uri.file(path.join(realSource, module)))
        .badge,
      "Ф",
    );
    await vscode.commands.executeCommand("ibcmd.forgetPassword");
    assert.deepEqual(errors, []);
    assert.ok(api.status().includes("ibcmd-rs"));
    // Opt-in actual native-mutated filedb READ smoke. This records byte-preview evidence,
    // not native-version parity: native may report version changes with unchanged bytes.
    const nativeRoot = process.env.IBCMD_TEST_FILEDB_ROOT;
    if (nativeRoot) {
      await cfg.update(
        "statusComparison",
        "native",
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      const filedb = path.join(nativeRoot, "db/1Cv8.1CD");
      const beforeRead = await fs.readFile(filedb);
      await fs.writeFile(
        path.join(f.uri.fsPath, "ibcmd-rs.toml"),
        await fs.readFile(path.join(nativeRoot, "workspace/ibcmd-rs.toml")),
      );
      await cfg.update(
        "connections",
        ["issue380-native"],
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "connectionMetadata",
        {},
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "sourceDirectory",
        path.join(nativeRoot, "source"),
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "statusBaselineDirectory",
        path.join(nativeRoot, "baseline"),
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      vscode.window.showQuickPick = async (items) => (await items)[0];
      await vscode.commands.executeCommand("ibcmd.select");
      await vscode.commands.executeCommand("ibcmd.objects.refresh");
      const actual = api.objects();
      const byName = new Map(actual.map((row) => [row.object, row.state]));
      assert.equal(byName.get("CommonModule.ФайловаяБазаПроверка"), "database");
      assert.equal(byName.get("Configuration"), "both");
      assert.equal(byName.get("Language.English"), "database");
      assert.equal(
        api.objectDecoration(
          vscode.Uri.file(
            path.join(
              nativeRoot,
              "source/CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl",
            ),
          ),
        ).badge,
        "Б",
      );
      assert.ok(
        api
          .objectDecoration(
            vscode.Uri.file(
              path.join(nativeRoot, "source/Languages/English.xml"),
            ),
          )
          .tooltip.includes("Версии БД и локальные файлы"),
      );
      const nativeStatus = await fs.readFile(
        path.join(nativeRoot, "module-status.stdout"),
        "utf8",
      );
      assert.ok(nativeStatus.includes("modified: Language.English"));
      assert.ok(nativeStatus.includes("modified: Configuration.Configuration"));
      await fs.writeFile(
        path.join(nativeRoot, "ui-evidence.json"),
        JSON.stringify(
          { labels: actual, nativeStatus, nativeVersionParity: true },
          null,
          2,
        ),
      );
      assert.deepEqual(
        await fs.readFile(filedb),
        beforeRead,
        "UI READ must not mutate the actual filedb",
      );
      // Same actual native comparison through a pinned Git baseline.
      const gitSource = path.join(f.uri.fsPath, "native-git-source");
      await fs.cp(path.join(nativeRoot, "baseline"), gitSource, {
        recursive: true,
      });
      const git = (...args) =>
        execFileSync("git", ["-C", gitSource, ...args], { stdio: "pipe" });
      git("init");
      git("add", ".");
      git(
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
        "commit",
        "-m",
        "native baseline",
      );
      await fs.appendFile(
        path.join(gitSource, "Configuration.xml"),
        "\n<!-- local Git source edit -->\n",
      );
      await cfg.update(
        "sourceDirectory",
        gitSource,
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "statusGitRef",
        "HEAD",
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "statusCacheDirectory",
        path.join(f.uri.fsPath, "native-git-cache"),
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await vscode.commands.executeCommand("ibcmd.select");
      await vscode.commands.executeCommand("ibcmd.objects.refresh");
      assert.deepEqual(
        api.objects(),
        actual,
        "Native folder and Git baseline states agree",
      );
      assert.deepEqual(
        await fs.readFile(filedb),
        beforeRead,
        "Git native refresh is read-only",
      );
      await cfg.update(
        "statusGitRef",
        "",
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      console.log(
        "Actual filedb native-version UI matches measured module/root/language changes; READ leaves DB bytes unchanged.",
      );
    }
    const sixRoot = process.env.IBCMD_TEST_SIXSTATE_ROOT;
    if (sixRoot) {
      const dbFile = path.join(sixRoot, "db/1Cv8.1CD");
      const before = await fs.readFile(dbFile);
      await fs.writeFile(
        path.join(f.uri.fsPath, "ibcmd-rs.toml"),
        await fs.readFile(path.join(sixRoot, "workspace/ibcmd-rs.toml")),
      );
      await cfg.update(
        "connections",
        ["issue380-native"],
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "connectionMetadata",
        {},
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "statusComparison",
        "native",
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "sourceDirectory",
        path.join(sixRoot, "source"),
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "statusBaselineDirectory",
        path.join(sixRoot, "baseline"),
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      await cfg.update(
        "statusGitRef",
        "",
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      vscode.window.showQuickPick = async (items) => (await items)[0];
      await vscode.commands.executeCommand("ibcmd.select");
      await vscode.commands.executeCommand("ibcmd.objects.refresh");
      const expected = JSON.parse(
        await fs.readFile(path.join(sixRoot, "expected-states.json"), "utf8"),
      );
      const labels = api.objects();
      const actual = new Map(labels.map((row) => [row.object, row.state]));
      const nativeStatus = await fs.readFile(
        path.join(sixRoot, "module-status.stdout"),
        "utf8",
      );
      const nativeNames = nativeStatus
        .trim()
        .split("\n")
        .filter(Boolean)
        .map((line) => line.replace(/^modified: /, ""));
      for (const [object, state] of Object.entries(expected)) {
        assert.equal(
          actual.get(object),
          state,
          `Actual native six-state ${object}: ${JSON.stringify(labels)}`,
        );
        const name = object.slice("CommonModule.".length);
        const decoration = api.objectDecoration(
          vscode.Uri.file(
            path.join(sixRoot, "source/CommonModules", name + ".xml"),
          ),
        );
        assert.ok(decoration, `Explorer decoration for ${object}`);
        if (["database", "both"].includes(state))
          assert.ok(
            nativeNames.includes(object) ||
              nativeNames.includes(object + ".Module"),
          );
        if (["same", "file"].includes(state))
          assert.ok(
            !nativeNames.includes(object) &&
              !nativeNames.includes(object + ".Module"),
          );
      }
      assert.deepEqual(
        [...new Set(Object.values(expected))].sort(),
        ["same", "database", "file", "both", "databaseOnly", "fileOnly"].sort(),
      );
      await vscode.commands.executeCommand("ibcmd.objects.changedOnly");
      assert.ok(api.visibleObjects().every((row) => row.state !== "same"));
      await vscode.commands.executeCommand("ibcmd.objects.changedOnly");
      assert.deepEqual(
        await fs.readFile(dbFile),
        before,
        "Six-state UI refresh never writes filedb",
      );
      await fs.writeFile(
        path.join(sixRoot, "ui-evidence.json"),
        JSON.stringify(
          {
            labels,
            nativeStatus,
            allSixStates: true,
            nativeVersionParity: true,
          },
          null,
          2,
        ),
      );
      console.log(
        "Actual native filedb: all SIX UI/Explorer states match independent native version/local file evidence; changed-only and read-only checks passed.",
      );
    }
    // Real extension UI/state machine against a clearly synthetic server, covering failure/races.
    const fake = path.join(__dirname, "fake-host-server.js");
    await fs.chmod(fake, 0o755);
    await cfg.update("executable", fake, vscode.ConfigurationTarget.Global);
    await cfg.update(
      "connectionMetadata",
      {
        "fake/a": { label: "Fake A", auth: "windows" },
        "fake/b": { label: "Fake B", auth: "windows" },
      },
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    vscode.window.showQuickPick = async (items, options) =>
      options?.canPickMany ? await items : (await items)[0];
    await vscode.commands.executeCommand("ibcmd.bind");
    assert.deepEqual(
      vscode.workspace.getConfiguration("ibcmd", f.uri).get("connections"),
      ["fake/a", "fake/b"],
    );
    let pick = "fake/a";
    vscode.window.showQuickPick = async (items) =>
      (await items).find((i) => i.base?.id === pick);
    for (const [mode, expected] of [
      ["unknown", "pending unknown"],
      ["pending", "pending changes"],
      ["error", "pending unknown"],
      ["crash", "disconnected"],
    ]) {
      process.env.IBCMD_FAKE_MODE = mode;
      await vscode.commands.executeCommand("ibcmd.select");
      assert.ok(api.status().includes(expected), `${mode}: ${api.status()}`);
      assert.ok(api.status().includes("8.5.1"), api.status());
    }
    // #380: real extension host and UI providers against explicitly synthetic six-state RPC.
    const source = path.join(f.uri.fsPath, "status-source");
    const baseline = path.join(f.uri.fsPath, "status-baseline");
    await fs.mkdir(source);
    await fs.mkdir(baseline);
    await fs.writeFile(
      path.join(baseline, "ConfigDumpInfo.xml"),
      "synthetic-native-reference",
    );
    await cfg.update(
      "statusComparison",
      "native",
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    for (const name of [
      "Same",
      "Database",
      "File",
      "Both",
      "DatabaseOnly",
      "FileOnly",
    ]) {
      await fs.writeFile(path.join(baseline, `${name}.xml`), "baseline");
      await fs.writeFile(
        path.join(source, `${name}.xml`),
        ["File", "Both"].includes(name) ? "local" : "baseline",
      );
    }
    await cfg.update(
      "sourceDirectory",
      source,
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    await cfg.update(
      "statusBaselineDirectory",
      baseline,
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    process.env.IBCMD_FAKE_MODE = "status";
    await vscode.commands.executeCommand("ibcmd.select");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.deepEqual(
      api.objects().map((r) => r.state),
      ["same", "database", "file", "both", "databaseOnly", "fileOnly"],
    );
    assert.equal(
      api.objectDecoration(vscode.Uri.file(path.join(source, "File.xml")))
        .badge,
      "Ф",
    );
    assert.equal(
      api.objectDecoration(vscode.Uri.file(path.join(baseline, "File.xml"))),
      undefined,
    );
    await vscode.commands.executeCommand("ibcmd.objects.changedOnly");
    assert.equal(api.visibleObjects().length, 5);
    await vscode.commands.executeCommand("ibcmd.objects.changedOnly");
    assert.equal(api.visibleObjects().length, 6);
    process.env.IBCMD_FAKE_MODE = "status-error";
    await vscode.commands.executeCommand("ibcmd.select");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.deepEqual(api.objects(), []);
    assert.equal(
      api.objectDecoration(vscode.Uri.file(path.join(source, "File.xml"))),
      undefined,
    );
    // Even contradictory clean row evidence cannot hide top-level fullRequired.
    process.env.IBCMD_FAKE_MODE = "status-full";
    await vscode.commands.executeCommand("ibcmd.select");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.ok(
      api
        .objects()
        .slice(0, 4)
        .every((row) => row.state === "unknown"),
    );
    // A cancelled refresh never publishes old clean decorations.
    process.env.IBCMD_FAKE_MODE = "status-slow";
    await vscode.commands.executeCommand("ibcmd.select");
    const progress = vscode.window.withProgress;
    vscode.window.withProgress = async (_, run) =>
      run(
        {},
        {
          isCancellationRequested: false,
          onCancellationRequested: (cb) => {
            const timer = setTimeout(cb, 20);
            return { dispose: () => clearTimeout(timer) };
          },
        },
      );
    try {
      await vscode.commands.executeCommand("ibcmd.objects.refresh");
    } finally {
      vscode.window.withProgress = progress;
    }
    assert.deepEqual(api.objects(), []);
    const stale = vscode.commands.executeCommand("ibcmd.objects.refresh");
    await new Promise((resolve) => setTimeout(resolve, 20));
    pick = "fake/b";
    process.env.IBCMD_FAKE_MODE = "status";
    await vscode.commands.executeCommand("ibcmd.select");
    await stale;
    assert.deepEqual(api.objects(), []);
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.equal(api.objects().length, 6);
    // No status leaks into a sibling workspace root, including identical relative paths.
    const sibling = path.join(path.dirname(f.uri.fsPath), "sibling");
    await fs.mkdir(sibling, { recursive: true });
    assert.equal(
      api.objectDecoration(vscode.Uri.file(path.join(sibling, "File.xml"))),
      undefined,
    );
    pick = "fake/a";
    // Apply UI consent path against the clearly synthetic own-backend contract.
    process.env.IBCMD_FAKE_MODE = "pending";
    await vscode.commands.executeCommand("ibcmd.select");
    const applyGate = path.join(f.uri.fsPath, "apply-consent-gate");
    process.env.IBCMD_FAKE_APPLY_GATE = applyGate;
    // Reconnect so the server inherits only this test's external gate path.
    await vscode.commands.executeCommand("ibcmd.select");
    vscode.window.showWarningMessage = async () => undefined;
    await vscode.commands.executeCommand("ibcmd.apply");
    await assert.rejects(fs.access(applyGate));
    vscode.window.showWarningMessage = async (message) => {
      assert.ok(message.includes("fake/a"));
      assert.ok(message.includes("exclusive"));
      return "Apply exclusively";
    };
    await vscode.commands.executeCommand("ibcmd.apply");
    assert.equal(await fs.readFile(applyGate, "utf8"), "applied");
    delete process.env.IBCMD_FAKE_APPLY_GATE;
    vscode.window.showWarningMessage = warning;
    console.log(
      "Apply UI: declined consent prevents execution; accepted consent invokes exclusive own RPC (synthetic only).",
    );
    process.env.IBCMD_FAKE_MODE = "slow";
    pick = "fake/a";
    const slow = vscode.commands.executeCommand("ibcmd.select");
    for (let i = 0; i < 100 && !api.status().includes("Fake A"); i++)
      await new Promise((r) => setTimeout(r, 10));
    assert.ok(api.status().includes("Fake A"), api.status());
    pick = "fake/b";
    await vscode.commands.executeCommand("ibcmd.select");
    await slow;
    assert.ok(api.status().includes("Fake B"));
    assert.ok(!api.status().includes("Fake A"));
    // Refresh must not restore A after a newer selection B completed while list was pending.
    process.env.IBCMD_FAKE_MODE = "pending";
    pick = "fake/a";
    await vscode.commands.executeCommand("ibcmd.select");
    assert.ok(api.status().includes("Fake A"), api.status());
    const gate = path.join(f.uri.fsPath, "refresh-list-gate");
    process.env.IBCMD_FAKE_MODE = "delayed-list";
    process.env.IBCMD_FAKE_LIST_GATE = gate;
    const refreshing = vscode.commands.executeCommand("ibcmd.refresh");
    let listPid;
    for (let i = 0; i < 200; i++) {
      try {
        listPid = Number(await fs.readFile(gate + ".started", "utf8"));
        break;
      } catch {}
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.ok(listPid, "Refresh list request reached the gate");
    process.env.IBCMD_FAKE_MODE = "pending";
    pick = "fake/b";
    await vscode.commands.executeCommand("ibcmd.select");
    assert.ok(api.status().includes("Fake B"), api.status());
    await fs.writeFile(gate, "release");
    await refreshing;
    assert.ok(
      api.status().includes("Fake B"),
      `Stale Refresh restored A: ${api.status()}`,
    );
    assert.ok(!api.status().includes("Fake A"), api.status());
    let alive = true;
    for (let i = 0; i < 200; i++) {
      try {
        process.kill(listPid, 0);
      } catch {
        alive = false;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.equal(alive, false, "The stale list process was disposed");
    // Synthetic login responses exercise actual commands and SecretStorage.
    // Next-selection prompt counts prove whether a new password was persisted.
    await cfg.update(
      "connectionMetadata",
      {
        "fake/a": { label: "Fake A", auth: "sql", user: "synthetic-user" },
        "fake/b": { label: "Fake B", auth: "windows" },
      },
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    let prompts = 0;
    const passwordInput = async () => {
      prompts++;
      return "synthetic-password-only";
    };
    vscode.window.showInputBox = passwordInput;
    const selectA = async (mode = "pending") => {
      pick = "fake/a";
      process.env.IBCMD_FAKE_MODE = mode;
      await vscode.commands.executeCommand("ibcmd.select");
    };
    let scenario = 0;
    const resetA = async () => {
      pick = "fake/a";
      process.env.IBCMD_FAKE_MODE = "pending";
      // Isolate scenarios by secret key; avoid cross-scenario SecretStorage cache events.
      await cfg.update(
        "connectionMetadata",
        {
          "fake/a": {
            label: "Fake A",
            auth: "sql",
            user: `synthetic-user-${++scenario}`,
          },
          "fake/b": { label: "Fake B", auth: "windows" },
        },
        vscode.ConfigurationTarget.WorkspaceFolder,
      );
      prompts = 0;
    };
    await selectA("login-error");
    await selectA();
    assert.equal(
      prompts,
      2,
      "Failed login must not cache the prompted password",
    );
    await vscode.commands.executeCommand("ibcmd.refresh");
    assert.equal(
      prompts,
      2,
      "Successful login persists for Select and Refresh",
    );
    await selectA("login-error");
    await selectA();
    assert.equal(
      prompts,
      2,
      "An existing stored password retains its behavior",
    );

    await resetA();
    vscode.window.showInputBox = async () => {
      prompts++;
      return undefined;
    };
    await selectA();
    vscode.window.showInputBox = passwordInput;
    await selectA();
    assert.equal(
      prompts,
      2,
      "Cancelled password prompt must not store a value",
    );

    await resetA();
    let releasePrompt, promptStarted;
    const prompting = new Promise((resolve) => {
      promptStarted = resolve;
    });
    vscode.window.showInputBox = () => {
      prompts++;
      promptStarted();
      return new Promise((resolve) => {
        releasePrompt = resolve;
      });
    };
    const stalePrompt = selectA();
    await prompting;
    pick = "fake/b";
    await vscode.commands.executeCommand("ibcmd.select");
    releasePrompt("synthetic-stale-password");
    await stalePrompt;
    assert.ok(api.status().includes("Fake B"), api.status());
    vscode.window.showInputBox = passwordInput;
    await selectA();
    assert.equal(prompts, 2, "A stale prompt must not persist its password");

    const waitForConnect = async (tag) => {
      const gate = path.join(f.uri.fsPath, tag);
      process.env.IBCMD_FAKE_CONNECT_GATE = gate;
      const request = selectA("connect-gate");
      let reached = false;
      for (let i = 0; i < 200; i++) {
        try {
          await fs.access(gate + ".started");
          reached = true;
          break;
        } catch {}
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
      assert.ok(reached, "Connect request reached the deterministic gate");
      return { request, gate };
    };
    await resetA();
    const staleConnect = await waitForConnect("stale-connect-gate");
    pick = "fake/b";
    process.env.IBCMD_FAKE_MODE = "pending";
    await vscode.commands.executeCommand("ibcmd.select");
    await fs.writeFile(staleConnect.gate, "release");
    await staleConnect.request;
    assert.ok(api.status().includes("Fake B"), api.status());
    await selectA();
    assert.equal(
      prompts,
      2,
      "A superseded connect must not persist its password",
    );

    await resetA();
    const cancellation = new vscode.CancellationTokenSource();
    vscode.window.withProgress = (_, task) => task({}, cancellation.token);
    const cancelled = await waitForConnect("cancelled-connect-gate");
    cancellation.cancel();
    await fs.writeFile(cancelled.gate, "release");
    await cancelled.request;
    cancellation.dispose();
    vscode.window.withProgress = progress;
    await selectA();
    assert.equal(prompts, 2, "Cancelled connect must not persist its password");
    // Rejected initialize on the connection process must reach the command error UI.
    // The list process initializes first; the marker rejects only the next process.
    process.env.IBCMD_FAKE_INITIALIZE_MARKER = path.join(
      f.uri.fsPath,
      "initialize-marker",
    );
    const beforeInitializeError = errors.length;
    await selectA("connect-initialize-error");
    assert.equal(
      errors.length,
      beforeInitializeError + 1,
      "Connection initialize error must be visible",
    );
    assert.ok(api.status().includes("disconnected"), api.status());
    delete process.env.IBCMD_FAKE_INITIALIZE_MARKER;
    process.env.IBCMD_FAKE_MODE = "pending";
    process.env.IBCMD_FAKE_INCLUDE_FILEDB = "1";
    const tomlFile = path.join(f.uri.fsPath, "ibcmd-rs.toml");
    const withFileDb =
      (await fs.readFile(tomlFile, "utf8")) +
      '\n[[database]]\nname="file"\nplatform="8.3.27"\ndb-path="protected-filedb"\n';
    await fs.writeFile(tomlFile, withFileDb);
    const metadataBeforeFileEdit = JSON.stringify(
      vscode.workspace
        .getConfiguration("ibcmd", f.uri)
        .get("connectionMetadata"),
    );
    let filePickerReached = false;
    const errorsBeforeFileEdit = errors.length;
    const selectionPicker = vscode.window.showQuickPick;
    vscode.window.showQuickPick = async (items) => {
      filePickerReached = true;
      const offered = await items;
      assert.ok(
        !offered.some((item) => item.base?.source === "file-db"),
        "File databases must be excluded from SQL Edit",
      );
      return undefined;
    };
    try {
      await vscode.commands.executeCommand("ibcmd.edit");
    } finally {
      vscode.window.showQuickPick = selectionPicker;
    }
    assert.equal(
      filePickerReached,
      true,
      "Edit must reach the source-filtered picker",
    );
    assert.equal(
      errors.length,
      errorsBeforeFileEdit,
      "Edit list/start must succeed",
    );
    assert.equal(
      await fs.readFile(tomlFile, "utf8"),
      withFileDb,
      "File-db TOML must stay byte-for-byte unchanged",
    );
    assert.equal(
      JSON.stringify(
        vscode.workspace
          .getConfiguration("ibcmd", f.uri)
          .get("connectionMetadata"),
      ),
      metadataBeforeFileEdit,
    );
    delete process.env.IBCMD_FAKE_INCLUDE_FILEDB;
    delete process.env.IBCMD_FAKE_CONNECT_GATE;
    console.log(
      "SQL password persistence: failed/cancelled/stale operations, successful reuse and existing-password behavior passed (synthetic login only).",
    );
    // Switch to another real workspace folder, then remove it without changing the first root.
    const sf = vscode.workspace.workspaceFolders.find(
      (w) => w.uri.fsPath === sibling,
    );
    assert.ok(sf);
    const siblingSource = path.join(sibling, "status-source");
    await fs.cp(source, siblingSource, { recursive: true });
    const scfg = vscode.workspace.getConfiguration("ibcmd", sf.uri);
    await scfg.update(
      "connections",
      ["fake/b"],
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    await scfg.update(
      "connectionMetadata",
      { "fake/b": { auth: "windows" } },
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    await scfg.update(
      "sourceDirectory",
      siblingSource,
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    await scfg.update(
      "statusBaselineDirectory",
      baseline,
      vscode.ConfigurationTarget.WorkspaceFolder,
    );
    pick = "fake/b"; // Reset the selection after the SQL password scenarios.
    vscode.window.showWorkspaceFolderPick = async () => sf;
    await vscode.commands.executeCommand("ibcmd.select");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.equal(api.objects().length, 6);
    assert.equal(
      api.objectDecoration(vscode.Uri.file(path.join(source, "File.xml"))),
      undefined,
    );
    assert.equal(
      api.objectDecoration(
        vscode.Uri.file(path.join(siblingSource, "File.xml")),
      ).badge,
      "Ф",
    );
    const changed = new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        sub.dispose();
        reject(new Error("Workspace removal event timed out"));
      }, 5000);
      const sub = vscode.workspace.onDidChangeWorkspaceFolders(() => {
        clearTimeout(timer);
        sub.dispose();
        resolve();
      });
    });
    assert.equal(vscode.workspace.updateWorkspaceFolders(sf.index, 1), true);
    await changed;
    assert.deepEqual(api.objects(), []);
    assert.equal(
      api.objectDecoration(
        vscode.Uri.file(path.join(siblingSource, "File.xml")),
      ),
      undefined,
    );
    delete process.env.IBCMD_FAKE_LIST_GATE;
    delete process.env.IBCMD_FAKE_MODE;
    console.log(
      "Extension host: connection lifecycle, real rows-dir object comparison/local file edit, synthetic six states, Explorer/filter, cancellation/stale selection and multi-root removal passed.",
    );
  } finally {
    vscode.window.showWorkspaceFolderPick = folderPick;
    vscode.window.showInputBox = input;
    vscode.window.showQuickPick = quick;
    vscode.window.showInformationMessage = info;
    vscode.window.showErrorMessage = error;
    vscode.window.withProgress = progress;
    vscode.window.showWarningMessage = warning;
    delete process.env.IBCMD_FAKE_APPLY_GATE;
    delete process.env.IBCMD_FAKE_CONNECT_GATE;
    delete process.env.IBCMD_FAKE_LIST_GATE;
    delete process.env.IBCMD_FAKE_MODE;
    delete process.env.IBCMD_FAKE_INCLUDE_FILEDB;
    delete process.env.IBCMD_FAKE_INITIALIZE_MARKER;
  }
};
