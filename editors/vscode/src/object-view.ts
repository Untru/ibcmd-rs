import * as vscode from "vscode";
import { createHash } from "node:crypto";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { RpcClient } from "./rpc";
import {
  candidatePaths,
  classify,
  classifyVersioned,
  comparisons,
  labels,
  localChanged,
  safePath,
  State,
} from "./object-status";
import { gitBaseline } from "./git-baseline";
import { Generation } from "./status";

// URI normalization handles drive spelling; Windows file identities ignore casing.
function fileKey(value: string): string {
  const normalized = vscode.Uri.file(value).fsPath;
  return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}
function decorationKey(uri: vscode.Uri): string {
  return uri.scheme === "file" ? "file:" + fileKey(uri.fsPath) : uri.toString();
}
function within(candidate: string, directory: string): boolean {
  const relative = path.relative(fileKey(directory), fileKey(candidate));
  return (
    relative === "" ||
    (!path.isAbsolute(relative) &&
      relative !== ".." &&
      !relative.startsWith(".." + path.sep))
  );
}

interface Row {
  object: string;
  state: State;
  uri: vscode.Uri;
}
export class ObjectView
  implements
    vscode.TreeDataProvider<Row>,
    vscode.FileDecorationProvider,
    vscode.Disposable
{
  private tree = new vscode.EventEmitter<Row | undefined>();
  private decorations = new vscode.EventEmitter<vscode.Uri[] | undefined>();
  readonly onDidChangeTreeData = this.tree.event;
  readonly onDidChangeFileDecorations = this.decorations.event;
  private rows: Row[] = [];
  private files = new Map<string, State>();
  private generation = new Generation();
  private abort?: AbortController;
  private roots: { descriptor: string; folder: string; state: State }[] = [];
  private changedOnly = false;
  private nativeMode = true;
  private view: vscode.TreeView<Row>;
  private message = "Выберите подключение и обновите статусы";
  constructor(private context: vscode.ExtensionContext) {
    const view = (this.view = vscode.window.createTreeView("ibcmd.objects", {
      treeDataProvider: this,
      canSelectMany: true,
    }));
    view.description = "Версии БД и локальные файлы";
    context.subscriptions.push(
      view,
      vscode.window.registerFileDecorationProvider(this),
      this,
    );
  }
  clear(message = "Статус неизвестен — требуется обновление") {
    this.generation.next();
    this.abort?.abort();
    this.rows = [];
    this.files.clear();
    this.roots = [];
    this.message = message;
    this.tree.fire(undefined);
    this.decorations.fire(undefined);
  }
  toggle() {
    this.changedOnly = !this.changedOnly;
    this.tree.fire(undefined);
  }
  snapshot() {
    return this.rows.map(({ object, state }) => ({ object, state }));
  }
  exportSelection(
    node?: { object?: string },
    selection?: { object?: string }[],
  ): string[] {
    const values = selection?.length
      ? selection
      : node
        ? [node]
        : this.view.selection;
    const available = new Set(this.rows.map((r) => r.object));
    return [
      ...new Set(
        values
          .map((r) => r.object)
          .filter(
            (name): name is string =>
              typeof name === "string" && available.has(name),
          ),
      ),
    ];
  }
  getTreeItem(row: Row) {
    const item = new vscode.TreeItem(row.object);
    item.contextValue = this.rows.includes(row)
      ? "ibcmd.object"
      : "ibcmd.message";
    item.description = labels[row.state];
    item.tooltip = `${row.object}: ${labels[row.state]}. ${this.nativeMode ? "Версии БД и локальные файлы относительно baseline." : "Сравнение байтов экспорта; native-версии не проверены."}`;
    item.resourceUri = row.uri;
    return item;
  }
  getChildren() {
    if (!this.rows.length)
      return [
        {
          object: this.message,
          state: "unknown" as State,
          uri: vscode.Uri.from({ scheme: "ibcmd-status", path: "/message" }),
        },
      ];
    return this.rows.filter((r) => !this.changedOnly || r.state !== "same");
  }
  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    const state =
      this.files.get(decorationKey(uri)) ??
      (uri.scheme === "file"
        ? this.roots.find(
            (r) =>
              fileKey(uri.fsPath) === fileKey(r.descriptor) ||
              within(uri.fsPath, r.folder),
          )?.state
        : undefined);
    if (!state) return;
    const badges: Record<State, string> = {
      same: "=",
      database: "Б",
      file: "Ф",
      both: "БФ",
      databaseOnly: "+Б",
      fileOnly: "+Ф",
      unknown: "?",
    };
    return {
      badge: badges[state],
      tooltip: `${labels[state]}. ${this.nativeMode ? "Версии БД и локальные файлы относительно baseline." : "Сравнение байтов экспорта; native-версии не проверены."}`,
      color: new vscode.ThemeColor(
        state === "same"
          ? "gitDecoration.untrackedResourceForeground"
          : state === "unknown"
            ? "disabledForeground"
            : "gitDecoration.modifiedResourceForeground",
      ),
    };
  }
  async refresh(
    rpc: RpcClient,
    folder: vscode.WorkspaceFolder,
    infobase: string,
  ) {
    this.clear("Обновление статусов…");
    const stamp = this.generation.capture();
    const abort = (this.abort = new AbortController());
    const cfg = vscode.workspace.getConfiguration("ibcmd", folder.uri);
    this.nativeMode =
      cfg.get<string>("statusComparison", "native") === "native";
    this.view.description = this.nativeMode
      ? "Версии БД и локальные файлы"
      : "Байты экспорта · preview";
    const source = path.resolve(
      folder.uri.fsPath,
      cfg.get<string>("sourceDirectory", "."),
    );
    const seconds = cfg.get<number>("statusTimeoutSeconds", 600);
    const statusTimeout =
      Number.isInteger(seconds) && seconds >= 30 && seconds <= 3600
        ? seconds * 1000
        : 600000;
    const base = cfg.get<string>("statusBaselineDirectory", "");
    let baseline = base ? path.resolve(folder.uri.fsPath, base) : undefined;
    try {
      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: "ibcmd-rs: статусы объектов",
          cancellable: true,
        },
        async (_, token) => {
          if (token.isCancellationRequested) abort.abort();
          const sub = token.onCancellationRequested(() => abort.abort());
          try {
            const refName = cfg.get<string>("statusGitRef", "");
            if (refName)
              baseline = await gitBaseline(
                source,
                refName,
                cfg.get<string>("statusCacheDirectory", ""),
                abort.signal,
              );
            for (const dir of [source, ...(baseline ? [baseline] : [])]) {
              const stat = await fs.lstat(dir);
              if (!stat.isDirectory() || stat.isSymbolicLink())
                throw new Error("Invalid source/baseline directory");
            }
            if (
              vscode.workspace.textDocuments.some(
                (d) =>
                  d.isDirty &&
                  d.uri.scheme === "file" &&
                  within(d.uri.fsPath, source),
              )
            )
              throw new Error("Save source files before comparing");
            let baselineIdentity: string | undefined;
            if (this.nativeMode && baseline) {
              const info = path.join(baseline, "ConfigDumpInfo.xml");
              if (!(await fs.lstat(info)).isFile())
                throw new Error("Native reference must be a regular file");
              baselineIdentity = createHash("sha256")
                .update(await fs.readFile(info))
                .digest("hex");
            }
            const response = await rpc.request(
              "objects/status",
              {
                infobase,
                sourceDir: source,
                ...(this.nativeMode && baseline
                  ? { versionBaselineDir: baseline }
                  : {}),
              },
              abort.signal,
              statusTimeout,
            );
            const current = comparisons(response);
            if (
              this.generation.current(stamp) &&
              !abort.signal.aborted &&
              this.nativeMode &&
              response.fullRequired === true
            )
              this.view.description =
                "Требуется полный экспорт · направление неизвестно";
            const reference = baseline
              ? comparisons(
                  await rpc.request(
                    "objects/status",
                    {
                      infobase,
                      sourceDir: baseline,
                      ...(this.nativeMode
                        ? { versionBaselineDir: baseline }
                        : {}),
                    },
                    abort.signal,
                    statusTimeout,
                  ),
                )
              : [];
            const byName = new Map(reference.map((r) => [r.object, r]));
            const paths = new Map<string, string>();
            const visited = new Set<string>();
            const visit = async (node?: string): Promise<void> => {
              const key = node ?? "";
              if (visited.has(key)) throw new Error("Cyclic tree response");
              visited.add(key);
              if (abort.signal.aborted) throw new Error("Cancelled");
              const result = await rpc.request(
                "tree/children",
                {
                  infobase,
                  // objects/status selects main only with versionBaselineDir.
                  mainConfiguration: this.nativeMode && !!baseline,
                  ...(node ? { node } : {}),
                },
                abort.signal,
              );
              if (!Array.isArray(result?.nodes))
                throw new Error("Invalid tree response");
              for (const child of result.nodes) {
                if (
                  typeof child.fullName === "string" &&
                  typeof child.path === "string"
                )
                  paths.set(child.fullName, safePath(source, child.path));
                if (child.hasChildren) await visit(child.id);
              }
            };
            await visit();
            const roots: {
              descriptor: string;
              folder: string;
              state: State;
            }[] = [];
            const rows: Row[] = [];
            const files = new Map<string, State>();
            for (const row of current) {
              if (abort.signal.aborted) throw new Error("Cancelled");
              const ref = byName.get(row.object);
              const changed =
                baseline && ref
                  ? await localChanged(source, baseline, row, ref)
                  : undefined;
              const state = this.nativeMode
                ? classifyVersioned(
                    row,
                    ref,
                    response.fullRequired === true ||
                      ref?.databaseVersionStatus?.conclusive !== true
                      ? undefined
                      : row.databaseVersionStatus,
                    changed,
                    baselineIdentity,
                    ref?.databaseVersionStatus?.databaseSnapshot,
                  )
                : classify(row, ref, changed);
              const uri = vscode.Uri.from({
                scheme: "ibcmd-status",
                authority: "object",
                path:
                  "/" +
                  encodeURIComponent(
                    folder.uri.toString() + "|" + infobase + "|" + row.object,
                  ),
              });
              rows.push({ object: row.object, state, uri });
              const descriptor = paths.get(row.object);
              if (descriptor)
                roots.push({
                  descriptor,
                  folder:
                    row.object === "Configuration"
                      ? path.join(source, "Ext")
                      : descriptor.replace(/\.xml$/, ""),
                  state,
                });
              files.set(uri.toString(), state);
              for (const p of candidatePaths(row, ref))
                files.set(
                  decorationKey(vscode.Uri.file(safePath(source, p))),
                  state,
                );
            }
            if (baselineIdentity && baseline) {
              const after = createHash("sha256")
                .update(
                  await fs.readFile(path.join(baseline, "ConfigDumpInfo.xml")),
                )
                .digest("hex");
              if (after !== baselineIdentity)
                throw new Error("Reference changed during refresh");
            }
            if (!this.generation.current(stamp) || abort.signal.aborted) return;
            this.rows = rows;
            this.files = files;
            this.roots = roots.sort(
              (a, b) => b.folder.length - a.folder.length,
            );
            this.message = "Нет объектов";
            this.tree.fire(undefined);
            this.decorations.fire(undefined);
          } finally {
            sub.dispose();
          }
        },
      );
    } catch {
      if (this.generation.current(stamp))
        this.clear(
          "Статус неизвестен — обновление отменено или завершилось ошибкой",
        );
    }
  }
  dispose() {
    this.clear();
    this.tree.dispose();
    this.decorations.dispose();
  }
}
