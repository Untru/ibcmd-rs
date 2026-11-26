// Real VS Code editor, real stdio server, own tiny file DB only; all fixtures retained.
const assert = require("node:assert/strict");
const vscode = require("vscode");
const fs = require("node:fs/promises");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { gunzipSync } = require("node:zlib");
const { RpcClient } = require("../out/rpc");
const { exportObjects } = require("../out/object-export");
exports.run = async () => {
  const folder = vscode.workspace.workspaceFolders[0];
  const root = folder.uri.fsPath;
  const repo = path.resolve(__dirname, "../../..");
  const fixture = path.join(repo, "tests/fixtures/file-infobase");
  const source = path.join(root, "source"),
    db = path.join(root, "db"),
    cache = path.join(root, "cache");
  await fs.mkdir(db);
  await fs.mkdir(cache);
  await fs.cp(path.join(fixture, "tiny-xml"), source, { recursive: true });
  const original = gunzipSync(
    await fs.readFile(path.join(fixture, "tiny.1CD.gz")),
  );
  await fs.writeFile(path.join(db, "1Cv8.1CD"), original);
  const moduleFile = path.join(
    source,
    "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl",
  );
  await fs.writeFile(moduleFile, "// local saved version\n");
  const git = (...args) => execFileSync("git", ["-C", source, ...args]);
  git("init", "-q");
  git("config", "user.email", "fixture@example.invalid");
  git("config", "user.name", "fixture");
  git("add", ".");
  git("commit", "-qm", "retained fixture");
  const cfg = vscode.workspace.getConfiguration("ibcmd", folder.uri);
  await cfg.update(
    "sourceDirectory",
    source,
    vscode.ConfigurationTarget.WorkspaceFolder,
  );
  await cfg.update(
    "exportCacheDirectory",
    cache,
    vscode.ConfigurationTarget.WorkspaceFolder,
  );
  const rpc = new RpcClient(process.env.IBCMD_TEST_BINARY, root);
  try {
    await rpc.request("initialize", {
      protocolVersion: "1.1",
      workspaceFolder: root,
      infobases: [{ name: "exportfixture", platform: "8.5.1", dbPath: db }],
    });
    const list = await rpc.request("infobases/list");
    const id = list.infobases.find((b) => b.name === "exportfixture").id;
    await rpc.request("infobases/connect", { infobase: id });
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(moduleFile),
    );
    await vscode.window.showTextDocument(doc);
    const edit = new vscode.WorkspaceEdit();
    edit.insert(doc.uri, new vscode.Position(0, 0), "// dirty buffer\n");
    await vscode.workspace.applyEdit(edit);
    assert.equal(doc.isDirty, true);
    await assert.rejects(
      exportObjects(
        rpc,
        folder,
        id,
        ["CommonModule.ФайловаяБазаПроверка"],
        false,
        () => true,
      ),
      /несохранённые/,
    );
    assert.equal(
      await fs.readFile(moduleFile, "utf8"),
      "// local saved version\n",
    );
    await vscode.commands.executeCommand("workbench.action.files.revert");
    assert.equal(doc.isDirty, false);
    assert.equal(
      await exportObjects(
        rpc,
        folder,
        id,
        ["Configuration/CommonModules"],
        false,
        () => true,
      ),
      true,
    );
    assert.deepEqual(
      await fs.readFile(moduleFile),
      await fs.readFile(
        path.join(
          fixture,
          "tiny-xml/CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl",
        ),
      ),
    );
    assert.deepEqual(
      await fs.readFile(path.join(source, "ConfigDumpInfo.xml")),
      await fs.readFile(path.join(fixture, "tiny-xml/ConfigDumpInfo.xml")),
    );
    assert.deepEqual(await fs.readFile(path.join(db, "1Cv8.1CD")), original);
    const extension = vscode.extensions.getExtension(
      "ibcmd-rs.ibcmd-rs-connections",
    );
    await extension.activate();
    const commands = await vscode.commands.getCommands(true);
    for (const command of [
      "ibcmd.objects.export",
      "ibcmd.objects.exportGroup",
      "ibcmd.objects.exportChanged",
    ])
      assert.ok(commands.includes(command), command);
    console.log(
      "ISSUE381_HOST_PASS dirty buffer refusal, real RPC group export, native bytes/inventory and DB immutability",
    );
  } finally {
    await rpc.dispose();
  }
};
