import * as fs from "node:fs";
import * as path from "node:path";
export function resolveExecutable(
  root: string,
  override = "",
  legacy = "",
  platform = process.platform,
): string {
  if (override.trim()) return override;
  if (legacy.trim()) return legacy;
  const bundled = path.join(
    root,
    "bin",
    platform === "win32" ? "ibcmd-rs.exe" : "ibcmd-rs",
  );
  return fs.existsSync(bundled) ? bundled : "ibcmd-rs";
}
