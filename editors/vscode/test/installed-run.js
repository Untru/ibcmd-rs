const fs = require("node:fs/promises");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { phase } = require("./installed-phase");
const { runTests, downloadAndUnzipVSCode } = require("@vscode/test-electron");
(async () => {
  const root = process.env.IBCMD_TEST_ROOT;
  if (!root || !process.env.IBCMD_TEST_VSIX || !process.env.IBCMD_TEST_ROWS)
    throw Error("Set IBCMD_TEST_ROOT, IBCMD_TEST_VSIX and IBCMD_TEST_ROWS");
  await fs.mkdir(root, { recursive: true });
  const run = await fs.mkdtemp(path.join(root, "installed-"));
  process.env.IBCMD_ACCEPTANCE_RUN = run;
  phase("prepare", { run });
  const workspace = path.join(run, "workspace"),
    driver = path.join(run, "driver");
  await fs.mkdir(workspace);
  await fs.mkdir(path.join(workspace, ".vscode"));
  await fs.writeFile(
    path.join(workspace, ".vscode", "settings.json"),
    JSON.stringify({
      "ibcmd.connections": ["lab/offline"],
      "ibcmd.connectionMetadata": { "lab/offline": { auth: "windows" } },
      "ibcmd.sourceDirectory": path.join(workspace, "source"),
      "ibcmd.statusComparison": "bytes",
    }),
  );
  await fs.mkdir(driver);
  await fs.writeFile(
    path.join(driver, "package.json"),
    JSON.stringify({
      name: "ibcmd-acceptance-driver",
      publisher: "lab",
      version: "0.0.1",
      engines: { vscode: "^1.95.0" },
    }),
  );
  phase("download-editor");
  const executable =
    process.env.VSCODE_EXECUTABLE_PATH ||
    (await downloadAndUnzipVSCode({
      cachePath: path.join(root, "vscode-download"),
      version: "1.95.3",
    }));
  const profile = [
    "--user-data-dir",
    path.join(run, "user-data"),
    "--extensions-dir",
    path.join(run, "extensions"),
  ];
  const editorEnv = { ...process.env };
  if (process.platform === "win32") {
    // Cursor also stores worktree state relative to the Windows home directory.
    const home = path.join(run, "home");
    Object.assign(editorEnv, {
      HOME: home,
      USERPROFILE: home,
      APPDATA: path.join(home, "AppData", "Roaming"),
      LOCALAPPDATA: path.join(home, "AppData", "Local"),
    });
    await fs.mkdir(editorEnv.APPDATA, { recursive: true });
    await fs.mkdir(editorEnv.LOCALAPPDATA, { recursive: true });
  }
  // Run Electron's JS CLI directly; .cmd wrappers cannot be execFile'd on Windows.
  const cliScript =
    process.env.IBCMD_TEST_EDITOR_CLI ||
    path.join(
      path.dirname(executable),
      ...(process.platform === "darwin" ? ["..", "Resources"] : ["resources"]),
      "app",
      "out",
      "cli.js",
    );
  phase("install-vsix", { executable });
  const installed = execFileSync(
    executable,
    [
      cliScript,
      ...profile,
      "--install-extension",
      path.resolve(process.env.IBCMD_TEST_VSIX),
      "--force",
    ],
    {
      encoding: "utf8",
      timeout: 120_000,
      killSignal: "SIGKILL",
      env: { ...editorEnv, ELECTRON_RUN_AS_NODE: "1" },
    },
  );
  console.log(installed);
  phase("launch-editor");
  await runTests({
    vscodeExecutablePath: executable,
    extensionDevelopmentPath: driver,
    extensionTestsPath: path.join(__dirname, "installed-suite.js"),
    launchArgs: [
      workspace,
      ...profile,
      "--disable-workspace-trust",
      "--disable-updates",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-telemetry",
      "--crash-reporter-directory",
      path.join(run, "crashes"),
    ],
    extensionTestsEnv: {
      ...editorEnv,
      IBCMD_INSTALLED_ROOT: path.join(run, "extensions"),
      IBCMD_ACCEPTANCE_RUN: run,
    },
  });
  // Some hosts exit successfully even when the test extension stops mid-suite.
  await fs.access(path.join(run, "acceptance.json"));
  phase("complete");
  console.log(`Retained installed-editor evidence: ${run}`);
})().catch((e) => {
  phase("failed", { message: String(e) });
  console.error(e);
  process.exitCode = 1;
});
