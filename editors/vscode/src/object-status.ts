import * as path from "node:path";
import * as fs from "node:fs/promises";

export type State =
  | "same"
  | "database"
  | "file"
  | "both"
  | "databaseOnly"
  | "fileOnly"
  | "unknown";
export interface Comparison {
  object: string;
  version?: string;
  databaseVersionStatus?: DatabaseVersionEvidence;
  status: "same" | "modified" | "notExported" | "notInBase";
  changed: string[];
  baseOnly: string[];
  localOnly: string[];
}
// Additive RPC evidence from the shared #362 native-version helper.
// objectVersion is the existing opaque RPC cache token; it is NOT configVersion or a file hash.
export interface DatabaseVersionEvidence {
  basis: "native-config-version";
  object: string;
  objectVersion: string;
  baselineIdentity: string;
  // Digest of the complete native inventory: freshness only, never change classification.
  databaseSnapshot: string;
  conclusive: boolean;
  changed?: boolean;
}

function direction(databaseChanged: boolean, fileChanged: boolean): State {
  return databaseChanged
    ? fileChanged
      ? "both"
      : "database"
    : fileChanged
      ? "file"
      : "same";
}

/** Combine independently measured DB version and local file evidence.
 * Callers must attach server evidence for this object/reference to this refresh;
 * never derive native evidence from byte equality or the RPC cache token itself.
 */
export function classifyVersioned(
  current: Comparison,
  reference: Comparison | undefined,
  evidence: DatabaseVersionEvidence | undefined,
  fileChanged?: boolean,
  expectedBaselineIdentity?: string,
  expectedDatabaseSnapshot?: string,
): State {
  if (current.status === "notExported") return "databaseOnly";
  if (current.status === "notInBase") return "fileOnly";
  if (
    !reference ||
    reference.object !== current.object ||
    !evidence ||
    typeof expectedBaselineIdentity !== "string" ||
    expectedBaselineIdentity.length === 0 ||
    evidence.baselineIdentity !== expectedBaselineIdentity ||
    typeof expectedDatabaseSnapshot !== "string" ||
    expectedDatabaseSnapshot.length === 0 ||
    evidence.databaseSnapshot !== expectedDatabaseSnapshot ||
    typeof fileChanged !== "boolean" ||
    evidence.basis !== "native-config-version" ||
    evidence.object !== current.object ||
    evidence.conclusive !== true ||
    typeof evidence.changed !== "boolean" ||
    typeof current.version !== "string" ||
    current.version.length === 0 ||
    current.version !== reference.version ||
    current.version !== evidence.objectVersion
  )
    return "unknown";
  return direction(evidence.changed, fileChanged);
}

export const labels: Record<State, string> = {
  same: "Совпадает",
  database: "Изменён в базе",
  file: "Изменён в файлах",
  both: "Изменён в базе и файлах",
  databaseOnly: "Только в базе",
  fileOnly: "Только в файлах",
  unknown: "Статус неизвестен",
};
export function classify(
  current: Comparison,
  baseline: Comparison | undefined,
  localChanged?: boolean,
): State {
  if (current.status === "notExported") return "databaseOnly";
  if (current.status === "notInBase") return "fileOnly";
  // Agreement alone is insufficient: both sides may have changed identically since baseline.
  if (!baseline || localChanged === undefined) return "unknown";
  if (
    typeof current.version !== "string" ||
    current.version.length === 0 ||
    current.version !== baseline.version
  )
    return "unknown";
  const dbChanged = baseline.status !== "same";
  return direction(dbChanged, localChanged);
}
export function comparisons(value: any): Comparison[] {
  if (!Array.isArray(value?.objects))
    throw new Error("Invalid object status response");
  const seen = new Set<string>();
  return value.objects.map((row: any) => {
    if (
      typeof row?.object !== "string" ||
      seen.has(row.object) ||
      !["same", "modified", "notExported", "notInBase"].includes(row.status) ||
      ![row.changed, row.baseOnly, row.localOnly].every(
        (a) => Array.isArray(a) && a.every((p) => typeof p === "string"),
      )
    )
      throw new Error("Invalid object status row");
    seen.add(row.object);
    return row as Comparison;
  });
}
export function safePath(root: string, relative: string): string {
  if (
    relative.includes("\\") ||
    relative.split("/").some((p) => p === ".." || p === "") ||
    path.isAbsolute(relative)
  )
    throw new Error("Unsafe source path");
  const resolved = path.resolve(root, relative);
  if (!resolved.startsWith(path.resolve(root) + path.sep))
    throw new Error("Unsafe source path");
  return resolved;
}
async function bytes(
  root: string,
  relative: string,
): Promise<Buffer | undefined> {
  const target = safePath(root, relative);
  try {
    // Reject symlinks in every path component, including missing/deleted files.
    let cursor = path.resolve(root);
    for (const part of relative.split("/")) {
      cursor = path.join(cursor, part);
      if ((await fs.lstat(cursor)).isSymbolicLink())
        throw new Error("Symlink source path");
    }
    return await fs.readFile(target);
  } catch (e: any) {
    if (e.code === "ENOENT") return undefined;
    throw e;
  }
}
export function candidatePaths(...rows: (Comparison | undefined)[]): string[] {
  return [
    ...new Set(
      rows.flatMap((r) =>
        r ? [...r.changed, ...r.baseOnly, ...r.localOnly] : [],
      ),
    ),
  ];
}
export async function localChanged(
  source: string,
  baseline: string,
  current: Comparison,
  reference: Comparison,
): Promise<boolean> {
  // Every local/baseline difference must differ from the DB in at least one comparison.
  // This union also detects additions, deletions and binary files without guessing FQN paths.
  let changed = false;
  for (const p of candidatePaths(current, reference)) {
    const a = await bytes(source, p);
    const b = await bytes(baseline, p);
    if (a === undefined ? b !== undefined : b === undefined || !a.equals(b))
      changed = true;
  }
  return changed;
}
