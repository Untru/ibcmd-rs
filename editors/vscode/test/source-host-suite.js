const vscode = require("vscode");
const path = require("node:path");
const os = require("node:os");
const fs = require("node:fs/promises");
const assert = require("node:assert/strict");
const { performance } = require("node:perf_hooks");
const { RpcClient } = require("../out/rpc");
exports.run = async () => {
  assert.ok(process.env.IBCMD_TEST_BINARY, "Укажите собранный сервер");
  const api = await vscode.extensions
    .getExtension("ibcmd-rs.ibcmd-rs-connections")
    .activate();
  const corpus = path.join(os.tmpdir(), "ibcmd-external-editor-server-export");
  const reference = path.join(corpus, "reference/demo");
  const local = await fs.mkdtemp(
    path.join(process.env.IBCMD_TEST_ROOT, "source-local-"),
  );
  await fs.cp(reference, local, { recursive: true });
  const workspace = vscode.workspace.workspaceFolders[0].uri.fsPath;
  const rpc = new RpcClient(process.env.IBCMD_TEST_BINARY, workspace);
  const errors = [];
  const error = vscode.window.showErrorMessage;
  const pick = vscode.window.showQuickPick;
  vscode.window.showErrorMessage = async (message) => {
    errors.push(message);
  };
  try {
    await rpc.request("initialize", {
      protocolVersion: "1.1",
      workspaceFolder: workspace,
      infobases: [
        {
          name: "пример",
          rowsDir: path.join(corpus, "rows/demo"),
          platform: "8.3.27",
        },
      ],
    });
    const bases = (await rpc.request("infobases/list")).infobases;
    const base = bases.find((b) => b.name === "пример");
    assert.ok(base);
    await rpc.request("infobases/connect", { infobase: base.id });
    api.configuration.connect(rpc, local, base.id);
    const referenceFiles = (
      await fs.readdir(reference, { recursive: true, withFileTypes: true })
    )
      .filter((entry) => entry.isFile())
      .map((entry) =>
        path
          .relative(reference, path.join(entry.parentPath, entry.name))
          .split(path.sep)
          .join("/"),
      )
      .filter((file) => file !== "ConfigDumpInfo.xml");
    const firstModule = referenceFiles.find((file) => file.endsWith(".bsl"));
    assert.ok(firstModule);
    const coldUri = api.databaseFiles.uri(
      api.configuration.source(),
      firstModule,
    );
    const coldStart = performance.now();
    const coldBytes = await vscode.workspace.fs.readFile(coldUri);
    const coldReadMs = performance.now() - coldStart;
    assert.deepEqual(
      Buffer.from(coldBytes),
      await fs.readFile(path.join(reference, firstModule)),
    );
    const nodes = [];
    const collect = async (node) => {
      if (node) nodes.push(node);
      for (const child of await api.configuration.getChildren(node))
        await collect(child);
    };
    const start = performance.now();
    await collect();
    const treeMs = performance.now() - start;
    assert.deepEqual(
      [...new Set(nodes.filter((n) => n.path).map((n) => n.path))].sort(),
      referenceFiles.sort(),
    );
    const module = nodes.find(
      (n) =>
        n.type === "file" &&
        n.path.endsWith(".bsl") &&
        n.parent.kind !== "ConfigurationFiles",
    );
    assert.ok(module, "В настоящем дереве есть модуль");
    const source = api.configuration.source(module);
    const uri = api.databaseFiles.uri(source, module.path);
    const expected = await fs.readFile(path.join(reference, module.path));
    const readStart = performance.now();
    assert.deepEqual(
      Buffer.from(await vscode.workspace.fs.readFile(uri)),
      expected,
    );
    const readMs = performance.now() - readStart;
    await vscode.commands.executeCommand("ibcmd.source.open", module);
    assert.equal(
      vscode.window.activeTextEditor.document.uri.scheme,
      "ibcmd-rs",
    );
    assert.equal(
      vscode.window.activeTextEditor.document.getText(),
      expected.toString("utf8").replace(/^\uFEFF/, ""),
    );
    await assert.rejects(
      vscode.workspace.fs.writeFile(uri, Buffer.from("нельзя")),
    );
    await assert.rejects(vscode.workspace.fs.delete(uri));
    assert.deepEqual(
      await fs.readFile(path.join(local, module.path)),
      expected,
    );

    await fs.appendFile(
      path.join(local, module.path),
      "\r\n// Местная проверка сравнения\r\n",
    );
    const oldFile = path.posix.join(
      path.posix.dirname(module.path),
      "МестныйМодуль.bsl",
    );
    await fs.writeFile(
      path.join(local, oldFile),
      "// Только в местной копии\r\n",
    );

    const collection = module.path.split("/")[0];
    const localObject = `${collection}/ТолькоМестный.xml`;
    const localModule = `${collection}/ТолькоМестный/Ext/Module.bsl`;
    await fs.writeFile(path.join(local, localObject), "<MetaDataObject/>\n");
    await fs.mkdir(path.dirname(path.join(local, localModule)), {
      recursive: true,
    });
    await fs.writeFile(
      path.join(local, localModule),
      "// Целиком местный объект\n",
    );
    const unrelated = "Catalogs/Посторонний.xml";
    await fs.mkdir(path.join(local, "Catalogs"), { recursive: true });
    await fs.writeFile(path.join(local, unrelated), "<MetaDataObject/>\n");

    let choices;
    vscode.window.showQuickPick = async (items) => {
      choices = await items;
      return choices.find((i) => i.difference?.path === module.path);
    };
    assert.equal(module.parent.parent.type, "group");
    await vscode.commands.executeCommand(
      "ibcmd.source.compare",
      module.parent.parent,
    );
    assert.ok(
      choices.some(
        (i) =>
          i.difference.path === module.path && i.difference.state === "changed",
      ),
    );
    assert.ok(
      choices.some(
        (i) =>
          i.difference.path === oldFile && i.difference.state === "localOnly",
      ),
    );

    for (const file of [localObject, localModule]) {
      assert.ok(
        choices.some(
          (i) =>
            i.difference.path === file && i.difference.state === "localOnly",
        ),
      );
    }
    assert.ok(choices.every((i) => i.difference.path !== unrelated));

    const diff = vscode.window.tabGroups.all
      .flatMap((g) => g.tabs)
      .find((t) => t.input instanceof vscode.TabInputTextDiff);
    assert.ok(diff);
    assert.equal(diff.input.original.scheme, "ibcmd-rs");
    assert.equal(
      diff.input.modified.fsPath,
      vscode.Uri.file(path.join(local, module.path)).fsPath,
    );
    assert.deepEqual(errors, []);
    const configFile = nodes.find(
      (n) => n.type === "file" && n.parent.kind === "ConfigurationFiles",
    );
    assert.ok(configFile);
    await fs.appendFile(path.join(local, configFile.path), "\r\n");
    choices = undefined;
    vscode.window.showQuickPick = async (items) => {
      choices = await items;
      return undefined;
    };
    await vscode.commands.executeCommand(
      "ibcmd.source.compare",
      configFile.parent,
    );
    assert.ok(choices.some((i) => i.difference.path === configFile.path));
    assert.ok(choices.every((i) => i.difference.path.startsWith("Ext/")));
    assert.deepEqual(errors, []);
    api.configuration.disconnect();
    await assert.rejects(vscode.workspace.fs.readFile(uri));
    console.log(
      JSON.stringify({
        проверка: "Настоящий сервер и VS Code",
        узлов: nodes.length,
        деревоМс: treeMs,
        чтениеМодуляМс: readMs,
        первоеЧтениеМодуляМс: coldReadMs,
        ERP: false,
      }),
    );
  } finally {
    vscode.window.showErrorMessage = error;
    vscode.window.showQuickPick = pick;
    await rpc.dispose();
  }
};
