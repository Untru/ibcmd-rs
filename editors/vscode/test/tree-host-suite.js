const vscode = require("vscode");
const path = require("node:path");
const fs = require("node:fs/promises");
const assert = require("node:assert/strict");
exports.run = async () => {
  const extension = vscode.extensions.getExtension(
    "ibcmd-rs.ibcmd-rs-connections",
  );
  assert.ok(extension);
  const api = await extension.activate();
  const workspace = vscode.workspace.workspaceFolders[0].uri.fsPath;
  const rootDir = path.join(workspace, "исходники");
  const open = vscode.window.showOpenDialog;
  try {
    vscode.window.showOpenDialog = async () => [vscode.Uri.file(rootDir)];
    await vscode.commands.executeCommand("ibcmd.tree.xml");
  } finally {
    vscode.window.showOpenDialog = open;
  }
  const [root] = await api.configuration.getChildren();
  const [general] = await api.configuration.getChildren(root);
  const groups = await api.configuration.getChildren(general);
  const [module] = await api.configuration.getChildren(
    groups.find((n) => n.kind === "CommonModule"),
  );
  const [file] = await api.configuration.getChildren(module);
  assert.equal(file.label, "Module.bsl");
  assert.equal(api.configuration.getParent(file), module);
  assert.equal(
    api.configuration.getTreeItem(file).contextValue,
    "ibcmd.source",
  );
  await vscode.commands.executeCommand("ibcmd.tree.openLocal", file);
  const doc = vscode.window.activeTextEditor.document;
  assert.equal(
    doc.uri.fsPath,
    vscode.Uri.file(path.join(rootDir, file.path)).fsPath,
  );
  const expected = (await fs.readFile(doc.uri.fsPath, "utf8")).replace(
    /^\uFEFF/,
    "",
  );
  assert.equal(doc.getText(), expected);
  assert.equal(api.configuration.getTreeItem(module).command, undefined);
  await fs.writeFile(path.join(workspace, "посторонний.xml"), "<проверка/>");
  await new Promise((resolve) => setTimeout(resolve, 1000));
  assert.equal((await api.configuration.getChildren())[0], root);
  let subscription;
  let timer;
  const changed = new Promise((resolve, reject) => {
    timer = setTimeout(
      () => reject(new Error("Изменение исходника не обновило дерево")),
      10000,
    );
    subscription = api.configuration.onDidChangeTreeData(() => resolve());
  });
  try {
    await fs.appendFile(
      path.join(rootDir, file.path),
      "\r\n// Проверка обновления дерева\r\n",
    );
    await changed;
  } finally {
    clearTimeout(timer);
    subscription.dispose();
  }
  assert.deepEqual(await api.configuration.getChildren(module), []);
  assert.notEqual((await api.configuration.getChildren())[0], root);
  await vscode.commands.executeCommand("ibcmd.tree.refresh");
  assert.deepEqual(await api.configuration.getChildren(module), []);
  assert.equal((await api.configuration.getChildren()).length, 1);
  console.log(
    "Проверка дерева в VS Code выполнена: раскрытие, команды, файл, границы наблюдения и обновление.",
  );
};
