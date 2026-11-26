const { phase } = require("./installed-phase");
const vscode = require("vscode");
const fs = require("node:fs/promises");
const path = require("node:path");
const assert = require("node:assert/strict");
const { createRequire } = require("node:module");
const { execFileSync } = require("node:child_process");
exports.run = async () => {
  phase("suite-start");
  const ext = vscode.extensions.getExtension("ibcmd-rs.ibcmd-rs-connections");
  assert.ok(ext, "VSIX extension must be installed");
  assert.ok(
    ext.extensionPath.startsWith(process.env.IBCMD_INSTALLED_ROOT),
    ext.extensionPath,
  );
  const binary = path.join(
    ext.extensionPath,
    "bin",
    process.platform === "win32" ? "ibcmd-rs.exe" : "ibcmd-rs",
  );
  const f = vscode.workspace.workspaceFolders[0],
    source = path.join(f.uri.fsPath, "source");
  await fs.mkdir(source);
  const cli = (...args) =>
    JSON.parse(
      execFileSync(binary, args, {
        cwd: f.uri.fsPath,
        encoding: "utf8",
        timeout: 60_000,
        killSignal: "SIGKILL",
      }),
    );
  const rows = process.env.IBCMD_TEST_ROWS;
  phase("fixture-inventory");
  const inventory = cli(
    "objects",
    "status",
    "--platform",
    "8.3.27",
    "--rows-dir",
    rows,
    "--source-dir",
    source,
  ).objects;
  assert.ok(inventory.length > 0);
  const args = [
    "objects",
    "export",
    "--platform",
    "8.3.27",
    "--rows-dir",
    rows,
    "--target-dir",
    source,
  ];
  for (const item of inventory) args.push("--object", item.object);
  phase("fixture-export");
  cli(...args);
  phase("write-fixture-config");
  await fs.writeFile(
    path.join(f.uri.fsPath, "ibcmd-rs.toml"),
    '[[database]]\nserver="lab"\nname="offline"\nplatform="8.3.27"\nrows-dir=' +
      JSON.stringify(rows) +
      "\n",
  );
  phase("configure-workspace");
  const cfg = vscode.workspace.getConfiguration("ibcmd", f.uri);
  phase("configure-connections");
  await cfg.update(
    "connections",
    ["lab/offline"],
    vscode.ConfigurationTarget.WorkspaceFolder,
  );
  phase("configure-connectionMetadata");
  await cfg.update(
    "connectionMetadata",
    { "lab/offline": { auth: "windows" } },
    vscode.ConfigurationTarget.WorkspaceFolder,
  );
  phase("configure-sourceDirectory");
  await cfg.update(
    "sourceDirectory",
    source,
    vscode.ConfigurationTarget.WorkspaceFolder,
  );
  phase("configure-statusComparison");
  await cfg.update(
    "statusComparison",
    "bytes",
    vscode.ConfigurationTarget.WorkspaceFolder,
  );
  phase("activate-extension");
  const api = await ext.activate();
  // VS Code caches its API by the requiring extension's identity. The external
  // driver has a different window namespace; patch the installed extension's API.
  const installedVscode = createRequire(
    path.join(ext.extensionPath, "dist", "extension.js"),
  )("vscode");
  phase("installed-api", { sharedWithDriver: installedVscode === vscode });
  const window = installedVscode.window;
  const quick = window.showQuickPick,
    error = window.showErrorMessage;
  let pickerSelections = 0;
  const errors = [];
  window.showQuickPick = async (items) => {
    const choices = await items;
    assert.deepEqual(
      choices.map((item) => item.base.id),
      ["lab/offline"],
    );
    pickerSelections++;
    phase("connection-picked", { id: choices[0].base.id, pickerSelections });
    return choices[0];
  };
  window.showErrorMessage = async (msg) => {
    errors.push(msg);
  };
  try {
    phase("select-connection");
    await vscode.commands.executeCommand("ibcmd.select");
    assert.equal(pickerSelections, 1);
    phase("refresh-tree");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.deepEqual(errors, []);
    assert.deepEqual(
      api
        .objects()
        .map((r) => r.object)
        .sort(),
      inventory.map((r) => r.object).sort(),
    );
    phase("visible-tree");
    assert.equal((await api.visibleObjects()).length, inventory.length);
    phase("configure-executable-override");
    const override = vscode.workspace.getConfiguration("ibcmd-rs");
    phase("write-executable-override");
    await override.update("path", binary, vscode.ConfigurationTarget.Global);
    phase("select-connection");
    await vscode.commands.executeCommand("ibcmd.select");
    assert.equal(pickerSelections, 2);
    phase("refresh-tree");
    await vscode.commands.executeCommand("ibcmd.objects.refresh");
    assert.deepEqual(
      api
        .objects()
        .map((r) => r.object)
        .sort(),
      inventory.map((r) => r.object).sort(),
    );
    assert.deepEqual(errors, []);
    phase("assertions-passed");
    const evidence = {
      editor: vscode.env.appName,
      version: vscode.version,
      installed: ext.extensionPath,
      binary,
      objects: api.objects(),
      override: true,
      pickerSelections,
    };
    await fs.writeFile(
      path.join(process.env.IBCMD_ACCEPTANCE_RUN, "acceptance.json"),
      JSON.stringify(evidence, null, 2),
    );
    console.log(JSON.stringify(evidence));
  } finally {
    window.showQuickPick = quick;
    window.showErrorMessage = error;
  }
};
