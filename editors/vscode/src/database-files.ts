import * as vscode from "vscode";
import { randomUUID } from "node:crypto";
import { DatabaseSource, Source, relativePath } from "./configuration-tree";

interface Entry {
  source: Source;
  file: string;
  empty: boolean;
}

export class DatabaseFiles
  implements vscode.FileSystemProvider, vscode.Disposable
{
  private changes = new vscode.EventEmitter<vscode.FileChangeEvent[]>();
  readonly onDidChangeFile = this.changes.event;
  private entries = new Map<string, Entry>();
  private abort = new AbortController();
  private session = randomUUID();
  private inflight = new Map<string, Promise<Uint8Array>>();

  uri(source: Source, file: string, empty = false): vscode.Uri {
    relativePath(file);
    const base = source instanceof DatabaseSource ? source.infobase : "XML";
    const uri = vscode.Uri.from({
      scheme: "ibcmd-rs",
      path: `/${encodeURIComponent(base)}/${file}`,
      query: `${this.session}${empty ? "-absent" : ""}`,
    });
    const key = uri.toString();
    const previous = this.entries.get(key);
    if (previous) {
      if (
        previous.source !== source ||
        previous.file !== file ||
        previous.empty !== empty
      )
        throw new Error("Источник виртуального файла изменился");
    } else this.entries.set(key, { source, file, empty });
    return uri;
  }
  invalidate() {
    this.abort.abort();
    this.abort = new AbortController();
    const events = [...this.entries.keys()].map((key) => ({
      type: vscode.FileChangeType.Deleted,
      uri: vscode.Uri.parse(key),
    }));
    this.entries.clear();
    this.inflight.clear();
    this.session = randomUUID();
    if (events.length) this.changes.fire(events);
  }
  watch(): vscode.Disposable {
    return new vscode.Disposable(() => {});
  }
  async stat(uri: vscode.Uri): Promise<vscode.FileStat> {
    const bytes = await this.readFile(uri);
    return {
      type: vscode.FileType.File,
      ctime: 0,
      mtime: 0,
      size: bytes.byteLength,
      permissions: vscode.FilePermission.Readonly,
    };
  }
  async readFile(uri: vscode.Uri): Promise<Uint8Array> {
    if (!vscode.workspace.isTrusted)
      throw vscode.FileSystemError.NoPermissions(
        "Рабочая область не является доверенной",
      );
    const key = uri.toString();
    const entry = this.entries.get(key);
    if (!entry) throw vscode.FileSystemError.FileNotFound(uri);
    if (entry.empty) return new Uint8Array();
    const signal = this.abort.signal;
    let read = this.inflight.get(key);
    if (!read) {
      read = (async () => {
        try {
          const bytes = await entry.source.read(entry.file, signal);
          if (signal.aborted || this.entries.get(key) !== entry)
            throw vscode.FileSystemError.Unavailable(uri);
          if (bytes === undefined)
            throw vscode.FileSystemError.FileNotFound(uri);
          return bytes;
        } catch (e) {
          if (e instanceof vscode.FileSystemError) throw e;
          throw vscode.FileSystemError.Unavailable(
            "Не удалось прочитать файл из базы",
          );
        }
      })();
      this.inflight.set(key, read);
    }
    try {
      return await read;
    } finally {
      if (this.inflight.get(key) === read) this.inflight.delete(key);
    }
  }
  readDirectory(uri: vscode.Uri): [string, vscode.FileType][] {
    throw vscode.FileSystemError.FileNotADirectory(uri);
  }
  createDirectory(): never {
    throw vscode.FileSystemError.NoPermissions(
      "Файлы базы доступны только для чтения",
    );
  }
  writeFile(): never {
    throw vscode.FileSystemError.NoPermissions(
      "Файлы базы доступны только для чтения",
    );
  }
  delete(): never {
    throw vscode.FileSystemError.NoPermissions(
      "Файлы базы доступны только для чтения",
    );
  }
  rename(): never {
    throw vscode.FileSystemError.NoPermissions(
      "Файлы базы доступны только для чтения",
    );
  }
  dispose() {
    this.invalidate();
    this.changes.dispose();
  }
}

export interface Difference {
  path: string;
  state: "changed" | "baseOnly" | "localOnly";
}
export function differences(
  response: any,
  includeObject: (name: string) => boolean = () => true,
): Difference[] {
  if (!Array.isArray(response?.objects))
    throw new Error("Неверный ответ сравнения");
  const files = new Map<string, Difference>();
  for (const object of response.objects) {
    if (!object || typeof object.object !== "string")
      throw new Error("Неверный объект сравнения");
    const include = includeObject(object.object);
    for (const state of ["changed", "baseOnly", "localOnly"] as const) {
      if (!Array.isArray(object[state])) throw new Error("Нет списка отличий");
      for (const file of object[state]) {
        if (typeof file !== "string") throw new Error("Неверный путь отличия");
        relativePath(file);
        if (!include) continue;
        const old = files.get(file);
        if (old && old.state !== state)
          throw new Error("Противоречивые результаты сравнения");
        files.set(file, { path: file, state });
      }
    }
  }
  return [...files.values()].sort((a, b) => a.path.localeCompare(b.path, "ru"));
}
