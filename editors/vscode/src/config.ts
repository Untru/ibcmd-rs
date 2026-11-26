import { parseForESLint, getStaticTOMLValue } from "toml-eslint-parser";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { randomUUID } from "node:crypto";

export interface Database {
  server?: string;
  name: string;
  platform: string;
  "rows-dir"?: string;
}
export const idOf = (d: Pick<Database, "server" | "name">) =>
  d.server?.trim() ? `${d.server.trim()}/${d.name.trim()}` : d.name.trim();
export function readConfig(text: string): Database[] {
  const value = getStaticTOMLValue(
    parseForESLint(text.replace(/^\uFEFF/, "")).ast,
  ) as { database?: Database[] };
  if (value.database !== undefined && !Array.isArray(value.database))
    throw new Error("database must use [[database]] tables");
  return value.database ?? [];
}
// JSON strings are TOML basic strings except JSON's optional \/ escape (not emitted).
const quote = (s: string) => JSON.stringify(s).replace(/\u007f/g, "\\u007F");
export function editConfig(
  text: string,
  oldId: string | undefined,
  database: Database,
): string {
  const required: (keyof Database)[] = ["name", "platform"];
  if (database["rows-dir"] !== undefined) required.push("rows-dir");
  for (const key of required) {
    if (typeof database[key] !== "string" || !database[key]!.trim())
      throw new Error(`${key} is required`);
  }
  if (database.name.includes("*"))
    throw new Error("New connections must name one database");
  const bases = readConfig(text);
  if (
    "db-path" in database ||
    (oldId && bases.some((b) => idOf(b) === oldId && "db-path" in b))
  )
    throw new Error(
      "File database connections cannot be changed by the SQL connection editor",
    );
  const matches = bases
    .map((b, i) => (idOf(b) === oldId ? i : -1))
    .filter((i) => i >= 0);
  if (oldId && matches.length !== 1)
    throw new Error(
      "Connection changed or has duplicate IDs; reload before editing",
    );
  if (bases.some((b, i) => idOf(b) === idOf(database) && i !== matches[0]))
    throw new Error("Connection ID already exists");
  if (!oldId) {
    const next =
      text +
      (text.endsWith("\n") || !text ? "" : "\n") +
      "\n[[database]]\n" +
      (["server", "name", "platform", "rows-dir"] as const)
        .filter((k) => database[k] !== undefined)
        .map((k) => `${k} = ${quote(database[k]!)}\n`)
        .join("");
    readConfig(next);
    return next;
  }
  const bom = text.startsWith("\uFEFF") ? 1 : 0;
  const ast = parseForESLint(text.slice(bom)).ast;
  const table = ast.body[0].body.find(
    (n) =>
      n.type === "TOMLTable" &&
      n.kind === "array" &&
      n.resolvedKey.length === 2 &&
      n.resolvedKey[0] === "database" &&
      n.resolvedKey[1] === matches[0],
  );
  if (!table || table.type !== "TOMLTable")
    throw new Error(
      "Editing inline database arrays is unsupported; use [[database]]",
    );
  const edits: { start: number; end: number; text: string }[] = [];
  const missing: string[] = [];
  const keys: (keyof Database)[] = ["server", "name", "platform"];
  if (database["rows-dir"] !== undefined) keys.push("rows-dir");
  for (const k of keys) {
    const field = table.body.find(
      (n) => n.key.keys.length === 1 && getStaticTOMLValue(n.key)[0] === k,
    );
    if (database[k] === undefined) {
      if (field)
        edits.push({
          start: field.range[0] + bom,
          end: field.range[1] + bom,
          text: "",
        });
    } else if (field)
      edits.push({
        start: field.value.range[0] + bom,
        end: field.value.range[1] + bom,
        text: quote(database[k]!),
      });
    else missing.push(`${k} = ${quote(database[k]!)}\n`);
  }
  if (missing.length)
    edits.push({
      start: table.range[1] + bom,
      end: table.range[1] + bom,
      text: "\n" + missing.join(""),
    });
  for (const e of edits.sort((a, b) => b.start - a.start))
    text = text.slice(0, e.start) + e.text + text.slice(e.end);
  readConfig(text);
  return text;
}
export async function snapshot(file: string): Promise<string> {
  try {
    const st = await fs.lstat(file);
    if (!st.isFile() || st.isSymbolicLink())
      throw new Error("Settings must be a regular file");
    return await fs.readFile(file, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return "";
    throw e;
  }
}
// Sibling lock serializes our writers; compare the exact snapshot again before atomic rename.
// External writers do not honor this lock: see README for the small OS-level CAS limitation.
export async function saveConfig(
  file: string,
  expected: string,
  next: string,
): Promise<void> {
  const lock = file + ".ibcmd-lock";
  const handle = await fs
    .open(lock, "wx", 0o600)
    .catch((error: NodeJS.ErrnoException) => {
      if (error.code === "EEXIST")
        throw new Error(
          "Another connection edit is in progress (settings lock exists)",
        );
      throw error;
    });
  const temp = path.join(path.dirname(file), `.ibcmd-${randomUUID()}.tmp`);
  let created = false;
  try {
    if ((await snapshot(file)) !== expected)
      throw new Error("Settings changed on disk; reload and retry");
    let mode = 0o600;
    try {
      mode = (await fs.stat(file)).mode & 0o777;
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
    }
    const out = await fs.open(temp, "wx", mode);
    created = true;
    try {
      await out.writeFile(next, "utf8");
      await out.sync();
    } finally {
      await out.close();
    }
    if ((await snapshot(file)) !== expected)
      throw new Error("Settings changed on disk; reload and retry");
    await fs.rename(temp, file);
    created = false;
  } finally {
    // These are exclusively our lock/temp artifacts, never existing user files.
    if (created) await fs.unlink(temp).catch(() => {});
    await handle.close();
    await fs.unlink(lock);
  }
}
