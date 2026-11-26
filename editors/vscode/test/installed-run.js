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
  // Run Electron's JS CLI directly; .cmd wrappers cannot be execFile'd on Windows.
  const cliScript = path.join(
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
      env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" },
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
      ...process.env,
      IBCMD_INSTALLED_ROOT: path.join(run, "extensions"),
      IBCMD_ACCEPTANCE_RUN: run,
    },
  });
  phase("complete");
  console.log(`Retained installed-editor evidence: ${run}`);
})().catch((e) => {
  phase("failed", { message: String(e) });
  console.error(e);
  process.exitCode = 1;
});
