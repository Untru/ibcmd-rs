import * as path from "node:path";
import { mkdir, mkdtemp } from "node:fs/promises";
import { spawn } from "node:child_process";
import { ImportRpc, ImportValidationError } from "./import";
import { serverEnvironment } from "./rpc";

export interface ApplyPlan {
  backend: "own" | "external-native";
  modes: string[];
  dbPath?: string;
  platform?: string;
  warning: string;
}
export interface ApplyUi {
  current(): Promise<boolean>;
  confirm(message: string): Promise<boolean>;
  log(message: string): void;
  native(plan: ApplyPlan, signal?: AbortSignal): Promise<void>;
}
export async function runApply(
  rpc: ImportRpc,
  infobase: string,
  recoveryDir: string,
  ui: ApplyUi,
  signal?: AbortSignal,
) {
  async function check() {
    if (signal?.aborted) throw new ImportValidationError("Apply cancelled");
    if (!(await ui.current()))
      throw new ImportValidationError(
        "Connection or settings changed; prepare apply again",
      );
  }
  await check();
  const plan: ApplyPlan = await rpc.request(
    "config/apply/prepare",
    { infobase },
    signal,
  );
  if (
    !plan.modes.includes("exclusive") ||
    !["own", "external-native"].includes(plan.backend)
  )
    throw new ImportValidationError("Unsupported apply backend or mode");
  await check();
  if (
    !(await ui.confirm(
      `Apply CONFIGSAVE to «${infobase}»? Backend: ${plan.backend}; mode: exclusive.\n${plan.warning}`,
    ))
  )
    return false;
  await check();
  ui.log(`Apply started (${plan.backend}, exclusive)`);
  try {
    if (plan.backend === "external-native") await ui.native(plan, signal);
    else
      await rpc.request(
        "config/apply",
        { infobase, mode: "exclusive", confirm: true, recoveryDir },
        signal,
        600000,
      );
    ui.log("Apply completed. Refreshing pending state.");
    return true;
  } catch (error) {
    ui.log(
      "Apply did not report success. Outcome may be uncertain; inspect pending state and recovery before retrying.",
    );
    throw error;
  }
}

// Native execution belongs to the client. Never use a shell or print native
// stdout/stderr: it can include paths, credentials or configuration contents.
export async function nativeApply(
  executable: string,
  plan: ApplyPlan,
  scratchRoot: string,
  signal?: AbortSignal,
  current: () => Promise<boolean> = async () => true,
): Promise<void> {
  if (!executable || !plan.dbPath || !plan.platform)
    return Promise.reject(
      new ImportValidationError(
        "Configure the bound platform's native executable first",
      ),
    );
  if (signal?.aborted)
    return Promise.reject(new ImportValidationError("Apply cancelled"));
  if (!path.isAbsolute(scratchRoot))
    throw new ImportValidationError(
      "Native scratch directory must be absolute",
    );
  await mkdir(scratchRoot, { recursive: true });
  const scratch = await mkdtemp(path.join(scratchRoot, "native-apply-"));
  if (!(await current()))
    throw new ImportValidationError(
      "Connection or settings changed; prepare apply again",
    );
  if (signal?.aborted) throw new ImportValidationError("Apply cancelled");
  return new Promise((resolve, reject) => {
    const env = serverEnvironment(process.env);
    const child = spawn(
      executable,
      [
        "infobase",
        "config",
        "apply",
        `--db-path=${plan.dbPath}`,
        "--dynamic=disable",
        `--data=${path.join(scratch, "data")}`,
        `--temp=${path.join(scratch, "temp")}`,
      ],
      { env, stdio: "ignore", windowsHide: true },
    );
    let killTimer: NodeJS.Timeout | undefined;
    let terminated = false;
    const cancel = () => {
      terminated = true;
      if (!killTimer) killTimer = setTimeout(() => child.kill("SIGKILL"), 5000);
      child.kill();
    };
    const timer = setTimeout(cancel, 600000);
    signal?.addEventListener("abort", cancel, { once: true });
    if (signal?.aborted) cancel();
    child.once("error", () => {
      clearTimeout(timer);
      clearTimeout(killTimer);
      signal?.removeEventListener("abort", cancel);
      reject(new ImportValidationError("Native apply failed to start"));
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      clearTimeout(killTimer);
      signal?.removeEventListener("abort", cancel);
      if (code === 0 && !signal?.aborted && !terminated) resolve();
      else
        reject(
          new ImportValidationError(
            "Native apply failed or was cancelled; inspect pending state before retrying",
          ),
        );
    });
  });
}
