const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
const vscode = require("vscode");

exports.run = async () => {
  const folder = vscode.workspace.workspaceFolders[0];
  const root = folder.uri.fsPath;
  const cfg = vscode.workspace.getConfiguration("ibcmd", folder.uri);
  const global = vscode.ConfigurationTarget.Global;
  await cfg.update("executable", process.env.IBCMD_TEST_BINARY, global);
  await cfg.update(
    "nativeExecutables",
    { "8.5.1": process.env.IBCMD_TEST_NATIVE },
    global,
  );
  await cfg.update(
    "connections",
    ["apply384"],
    vscode.ConfigurationTarget.Workspace,
  );
  await cfg.update(
    "sourceDirectory",
    "edited",
    vscode.ConfigurationTarget.Workspace,
  );
  const api = await vscode.extensions
    .getExtension("ibcmd-rs.ibcmd-rs-connections")
    .activate();
  const ui = vscode.window;
  const { showErrorMessage, showQuickPick, showWarningMessage } = ui;
  const errors = [];
  const confirmations = [];
  let acceptApply = false;
  ui.showErrorMessage = async (message) => {
    errors.push(message);
  };
  ui.showQuickPick = async (items) =>
    (await items).find((item) => item.base?.id === "apply384");
  ui.showWarningMessage = async (message, options, button) => {
    assert.equal(options.modal, true);
    confirmations.push(button);
    if (button === "Загрузить") return button;
    assert.match(message, /external-native.*exclusive/);
    return acceptApply ? button : undefined;
  };
  async function command(name, ...args) {
    await vscode.commands.executeCommand(name, ...args);
    assert.deepEqual(errors, []);
  }
  try {
    await command("ibcmd.select");
    assert.match(api.status(), / · no pending changes$/);
    const selected = vscode.Uri.joinPath(
      folder.uri,
      "edited",
      ...process.env.IBCMD_HOST_MODULE.split("/"),
    );
    await command("ibcmd.importSelected", selected);
    assert.match(api.status(), / · pending changes$/);
    const database = path.join(root, "disposable", "1Cv8.1CD");
    const staged = await fs.readFile(database);
    await command("ibcmd.apply");
    assert.deepEqual(await fs.readFile(database), staged);
    assert.match(api.status(), / · pending changes$/);
    acceptApply = true;
    await command("ibcmd.apply");
    assert.match(api.status(), / · no pending changes$/);
    assert.deepEqual(confirmations, [
      "Загрузить",
      "Apply exclusively",
      "Apply exclusively",
    ]);
  } finally {
    Object.assign(ui, { showErrorMessage, showQuickPick, showWarningMessage });
  }
};
