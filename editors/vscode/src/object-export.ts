import * as vscode from "vscode";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { RpcClient } from "./rpc";
import { safePath } from "./object-status";
const exec = promisify(execFile);
export interface ExportPlan {
  objects: string[];
  files: string[];
  removed: string[];
  expected: Record<string, string | null>;
}
const digest = (bytes: Buffer) =>
  createHash("sha256").update(bytes).digest("hex");
export async function plainPath(file: string): Promise<void> {
  const parent = path.dirname(file);
  if (parent !== file) await plainPath(parent);
  try {
    const stat = await fs.lstat(file);
    if (
      stat.isSymbolicLink() ||
      (stat.isFile() && stat.nlink > 1) ||
      (!stat.isDirectory() && !stat.isFile())
    )
      throw new Error(
        `Export refuses linked, hard-linked or special path: ${file}`,
      );
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
  }
}
export async function insensitiveSource(root: string): Promise<boolean> {
  if (process.platform === "win32") return true;
  // Probe existing path identities without creating files in the source tree.
  for (
    let part = root;
    path.dirname(part) !== part;
    part = path.dirname(part)
  ) {
    const base = path.basename(part);
    const flipped = base.replace(/[a-zA-Z]/, (c) =>
      c === c.toLowerCase() ? c.toUpperCase() : c.toLowerCase(),
    );
    if (base === flipped) continue;
    try {
      const a = await fs.stat(part),
        b = await fs.stat(path.join(path.dirname(part), flipped));
      return a.dev === b.dev && a.ino === b.ino;
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
      return false;
    }
  }
  return false;
}
export function targetIdentity(file: string, insensitive: boolean): string {
  const resolved = path.resolve(file);
  return insensitive ? resolved.toLowerCase() : resolved;
}
export function uniqueTargets(
  source: string,
  plan: ExportPlan,
  insensitive: boolean,
): void {
  const identities = new Set<string>();
  for (const file of [...plan.files, ...plan.removed]) {
    const key = targetIdentity(safePath(source, file), insensitive);
    if (identities.has(key))
      throw new Error(
        "Export targets alias on this filesystem; resolve case-only renames before export",
      );
    identities.add(key);
  }
}
export async function sourceFingerprint(
  file: string,
): Promise<string | undefined> {
  await plainPath(file);
  try {
    return digest(await fs.readFile(file));
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw e;
  }
}
export async function gitConflicts(
  root: string,
  affected: string[],
  signal: AbortSignal,
): Promise<string[]> {
  const git = async (...args: string[]) =>
    (
      await exec("git", ["-C", root, ...args], {
        signal,
        env: { ...process.env, LC_ALL: "C" },
        maxBuffer: 64 * 1024 * 1024,
      })
    ).stdout;
  let rawPrefix: string;
  try {
    rawPrefix = await git("rev-parse", "--show-prefix");
  } catch (error) {
    const e = error as { code?: number; stderr?: string };
    if (e.code !== 128 || !e.stderr?.includes("not a git repository"))
      throw error;
    const existing: string[] = [];
    for (const file of affected)
      if ((await sourceFingerprint(safePath(root, file))) !== undefined)
        existing.push(file);
    return existing;
  }
  const prefix = rawPrefix.replace(/\r?\n$/, "");
  const lists = await Promise.all([
    git(
      "ls-files",
      "--full-name",
      "-z",
      "--modified",
      "--deleted",
      "--others",
      "--exclude-standard",
    ),
    git(
      "ls-files",
      "--full-name",
      "-z",
      "--others",
      "--ignored",
      "--exclude-standard",
    ),
    git("diff", "--no-relative", "--name-only", "--cached", "-z", "--"),
  ]);
  const insensitive = await insensitiveSource(root);
  const key = (name: string) => (insensitive ? name.toLowerCase() : name);
  const changed = new Set(
    lists.flatMap((s) => s.split("\0").filter(Boolean)).map(key),
  );
  return affected.filter((file) => changed.has(key(prefix + file)));
}
export function validatePlan(value: unknown): ExportPlan {
  const p = value as ExportPlan;
  if (
    !p ||
    ![p.objects, p.files, p.removed].every(
      (a) => Array.isArray(a) && a.every((v) => typeof v === "string"),
    )
  )
    throw new Error("Invalid export plan");
  const all = [...p.files, ...p.removed];
  if (new Set(all).size !== all.length)
    throw new Error("Duplicate export target");
  if (
    !p.expected ||
    typeof p.expected !== "object" ||
    Array.isArray(p.expected)
  )
    throw new Error("Missing export fingerprints");
  for (const file of all) {
    if (
      !Object.hasOwn(p.expected, file) ||
      (p.expected[file] !== null && !/^[a-f0-9]{64}$/.test(p.expected[file]))
    )
      throw new Error("Invalid export fingerprint");
    safePath(path.resolve("/export-validation"), file);
    if (file.split("/").some((p) => p.startsWith(".")))
      throw new Error("Hidden export target");
  }
  return p;
}
export async function exportObjects(
  rpc: RpcClient,
  folder: vscode.WorkspaceFolder,
  infobase: string,
  selectors: string[],
  changed: boolean,
  current: () => boolean,
): Promise<boolean> {
  const cfg = vscode.workspace.getConfiguration("ibcmd", folder.uri);
  const source = path.resolve(
    folder.uri.fsPath,
    cfg.get<string>("sourceDirectory", "."),
  );
  const cache = cfg.get<string>("exportCacheDirectory", "");
  if (!path.isAbsolute(cache))
    throw new Error(
      "Set an absolute ibcmd.exportCacheDirectory for retained export staging files.",
    );
  // A staging directory under source would invalidate the source snapshot.
  const relative = path.relative(source, cache);
  if (
    relative === "" ||
    (!relative.startsWith(".." + path.sep) &&
      relative !== ".." &&
      !path.isAbsolute(relative))
  )
    throw new Error("Export cache must be outside sourceDirectory");
  await plainPath(cache);
  await fs.mkdir(cache, { recursive: true });
  const stage = await fs.mkdtemp(path.join(cache, "ibcmd-export-"));
  return vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "ibcmd-rs: выгрузка из базы",
      cancellable: true,
    },
    async (_, token) => {
      const abort = new AbortController();
      if (token.isCancellationRequested) abort.abort();
      const sub = token.onCancellationRequested(() => abort.abort());
      const check = () => {
        if (!current() || abort.signal.aborted || !vscode.workspace.isTrusted)
          throw new Error("Export cancelled or connection changed");
      };
      try {
        await plainPath(source);
        check();
        const seconds = cfg.get<number>("statusTimeoutSeconds", 600);
        const plan = validatePlan(
          await rpc.request(
            "objects/exportPlan",
            {
              infobase,
              objects: selectors,
              changed,
              sourceDir: source,
              targetDir: stage,
            },
            abort.signal,
            Number.isInteger(seconds) && seconds >= 30 && seconds <= 3600
              ? seconds * 1000
              : 600000,
          ),
        );
        const insensitive = await insensitiveSource(source);
        uniqueTargets(source, plan, insensitive);
        const before = new Map(
          Object.entries(plan.expected).map(([name, hash]) => [
            name,
            hash ?? undefined,
          ]),
        );
        check();
        // Avoid touching equal bytes; still protect ConfigDumpInfo whenever it changes.
        const staged = new Map<string, Buffer>();
        for (const file of plan.files) {
          const target = safePath(stage, file);
          await plainPath(target);
          const bytes = await fs.readFile(target);
          if (before.get(file) !== digest(bytes)) staged.set(file, bytes);
        }
        const removed = plan.removed.filter(
          (file) => before.get(file) !== undefined,
        );
        const affected = [...staged.keys(), ...removed];
        if (!affected.length) {
          void vscode.window.showInformationMessage(
            "Нет изменений для выгрузки.",
          );
          return false;
        }
        const dirty = () =>
          vscode.workspace.textDocuments
            .filter(
              (d) =>
                d.isDirty &&
                d.uri.scheme === "file" &&
                affected.some((file) => {
                  const target = safePath(source, file);
                  return (
                    targetIdentity(target, insensitive) ===
                    targetIdentity(d.uri.fsPath, insensitive)
                  );
                }),
            )
            .map((d) => path.relative(source, d.uri.fsPath));
        const ensureDirty = () => {
          const list = dirty();
          if (list.length)
            throw new Error(
              `Сохраните или отмените несохранённые правки перед выгрузкой:\n${list.join("\n")}`,
            );
        };
        ensureDirty();
        for (const file of affected) await plainPath(safePath(source, file));
        const conflicts = await gitConflicts(source, affected, abort.signal);
        check();
        if (conflicts.length) {
          const choice = await vscode.window.showWarningMessage(
            `Выгрузка перезапишет локальные изменения Git:\n${conflicts.join("\n")}`,
            { modal: true },
            "Перезаписать перечисленные файлы",
          );
          if (choice !== "Перезаписать перечисленные файлы") return false;
        }
        check();
        ensureDirty();
        const unchanged = async (file: string) => {
          if (
            (await sourceFingerprint(safePath(source, file))) !==
            before.get(file)
          )
            throw new Error(
              `Файл изменён во время выгрузки: ${file}. Повторите команду.`,
            );
        };
        for (const file of affected) await unchanged(file);
        // Recheck Git after the prompt; newly staged/modified paths need a new decision.
        const latest = await gitConflicts(source, affected, abort.signal);
        if (latest.some((file) => !conflicts.includes(file)))
          throw new Error("Git state changed during export; retry");
        check();
        ensureDirty();
        // Preflight every target before the first write, including parent links.
        for (const file of affected) {
          const target = safePath(source, file);
          await plainPath(target);
          try {
            if (!(await fs.lstat(target)).isFile())
              throw new Error(`Export target is not a regular file: ${file}`);
          } catch (e) {
            if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
          }
        }
        // Keep inventory last so interrupted publication cannot advertise completion.
        const writes = [...staged].sort(
          ([a], [b]) =>
            Number(a === "ConfigDumpInfo.xml") -
            Number(b === "ConfigDumpInfo.xml"),
        );
        // Keep original bytes beside the retained plan before changing disk files.
        const backup = path.join(stage, "originals");
        for (const file of affected) {
          await unchanged(file);
          if (before.get(file) !== undefined) {
            const target = safePath(backup, file);
            await fs.mkdir(path.dirname(target), { recursive: true });
            await fs.copyFile(safePath(source, file), target);
          }
        }
        for (const file of removed) {
          await unchanged(file);
          check();
          ensureDirty();
          await vscode.workspace.fs.delete(
            vscode.Uri.file(safePath(source, file)),
            { useTrash: true },
          );
        }
        for (const [file, bytes] of writes) {
          check();
          ensureDirty();
          const target = safePath(source, file);
          await fs.mkdir(path.dirname(target), { recursive: true });
          await unchanged(file);
          check();
          ensureDirty();
          await fs.writeFile(target, bytes);
        }
        void vscode.window.showInformationMessage(
          `Выгружено объектов: ${plan.objects.length}; файлов: ${staged.size}; удалено в корзину: ${removed.length}.`,
        );
        return true;
      } finally {
        sub.dispose();
      }
    },
  );
}
