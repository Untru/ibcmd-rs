import * as vscode from "vscode";
import * as path from "node:path";
import { ConfigurationView } from "./configuration-view";
import { DatabaseFiles, Difference, differences } from "./database-files";
import {
  DatabaseSource,
  SOURCE_OPERATION_TIMEOUT,
  Node,
  XmlSource,
  groupContains,
  localPath,
} from "./configuration-tree";

class Notice extends Error {}

export function registerDatabaseCommands(
  context: vscode.ExtensionContext,
  view: ConfigurationView,
): DatabaseFiles {
  const files = new DatabaseFiles();
  context.subscriptions.push(
    files,
    vscode.workspace.registerFileSystemProvider("ibcmd-rs", files, {
      isCaseSensitive: true,
      isReadonly: true,
    }),
    view.onDidChangeSource(() => files.invalidate()),
  );
  const command = (id: string, run: (node?: Node) => Promise<void>) =>
    context.subscriptions.push(
      vscode.commands.registerCommand(id, async (node?: Node) => {
        if (!vscode.workspace.isTrusted) return;
        try {
          await run(node);
        } catch (e) {
          void vscode.window.showErrorMessage(
            e instanceof Notice
              ? e.message
              : "Не удалось прочитать или сравнить исходники. Проверьте подключение и обновите дерево.",
          );
        }
      }),
    );
  function source(node?: Node): DatabaseSource {
    const current = node && view.source(node);
    if (!(current instanceof DatabaseSource))
      throw new Notice("Выберите объект подключённой базы");
    return current;
  }
  command("ibcmd.source.open", async (node) => {
    const current = source(node);
    if (!node?.path) throw new Error("Выберите файл или объект");
    await vscode.commands.executeCommand(
      "vscode.open",
      files.uri(current, node.path),
    );
  });
  async function showDiff(
    current: DatabaseSource,
    difference: Difference,
    node: Node,
  ) {
    if (view.source(node) !== current)
      throw new Error("Подключение изменилось");
    const local = new XmlSource(current.directory);
    const buffer = await local.read(
      difference.path,
      new AbortController().signal,
    );
    if (view.source(node) !== current)
      throw new Error("Подключение изменилось");
    const left = files.uri(
      current,
      difference.path,
      difference.state === "localOnly",
    );
    const right =
      buffer === undefined
        ? files.uri(current, difference.path, true)
        : vscode.Uri.file(localPath(current.directory, difference.path));
    await vscode.commands.executeCommand(
      "vscode.diff",
      left,
      right,
      `${difference.path}: база ↔ местный файл`,
    );
  }
  command("ibcmd.source.compare", async (node) => {
    const current = source(node);
    if (!node) return;
    if (node.type === "file" && node.path) {
      await showDiff(current, { path: node.path, state: "changed" }, node);
      return;
    }
    const result = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Сравнение с базой",
        cancellable: true,
      },
      async (_, token) => {
        const abort = new AbortController();
        if (token.isCancellationRequested) abort.abort();
        const cancel = token.onCancellationRequested(() => abort.abort());
        const changed = view.onDidChangeSource(() => abort.abort());
        try {
          const group =
            node.type === "group" && node.kind !== "ConfigurationFiles";
          const names = group
            ? undefined
            : await view.objectNames(node, abort.signal);
          if (names && !names.length) return [];
          const sourceDir = path.resolve(current.directory);
          if (
            vscode.workspace.textDocuments.some((d) => {
              if (!d.isDirty || d.uri.scheme !== "file") return false;
              const p = path.relative(sourceDir, d.uri.fsPath);
              return (
                p === "" ||
                (!path.isAbsolute(p) &&
                  p !== ".." &&
                  !p.startsWith(".." + path.sep))
              );
            })
          )
            throw new Notice(
              "Сохраните местные изменения перед сравнением группы",
            );
          const result = differences(
            await current.rpc.request(
              "objects/status",
              {
                infobase: current.infobase,
                ...(names ? { objects: names } : {}),
                sourceDir,
              },
              abort.signal,
              group ? SOURCE_OPERATION_TIMEOUT : undefined,
            ),
            group ? (name) => groupContains(node, name) : undefined,
          );
          return node.kind === "ConfigurationFiles"
            ? result.filter((d) => d.path.startsWith("Ext/"))
            : result;
        } finally {
          cancel.dispose();
          changed.dispose();
        }
      },
    );
    if (view.source(node) !== current) return;
    if (!result.length) {
      void vscode.window.showInformationMessage("Отличий от базы нет.");
      return;
    }
    const labels = {
      changed: "Изменён",
      baseOnly: "Только в базе",
      localOnly: "Только в местных файлах",
    };
    const selected = await vscode.window.showQuickPick(
      result.map((d) => ({
        label: d.path,
        description: labels[d.state],
        difference: d,
      })),
      { placeHolder: "Выберите файл для сравнения" },
    );
    if (selected) await showDiff(current, selected.difference, node);
  });
  return files;
}
