const path = require("node:path");
const fs = require("node:fs/promises");
const { runTests } = require("@vscode/test-electron");
(async () => {
  const root = process.env.IBCMD_TEST_ROOT;
  if (!root)
    throw new Error("Set IBCMD_TEST_ROOT to an isolated test/cache directory");
  if (!process.env.IBCMD_TEST_BINARY)
    throw new Error("Set IBCMD_TEST_BINARY to the compiled ibcmd-rs");
  await fs.mkdir(root, { recursive: true });
  const run = await fs.mkdtemp(path.join(root, "host-"));
  const workspace = path.join(run, "workspace");
  await fs.mkdir(workspace);
  const sibling = path.join(run, "sibling");
  await fs.mkdir(sibling);
  const workspaceFile = path.join(run, "test.code-workspace");
  await fs.writeFile(
    workspaceFile,
    JSON.stringify({ folders: [{ path: workspace }, { path: sibling }] }),
  );
  const rows = process.env.IBCMD_TEST_ROWS;
  if (!rows)
    throw new Error("Set IBCMD_TEST_ROWS (see README test instructions)");
  const executable = process.env.VSCODE_EXECUTABLE_PATH;
  await runTests({
    extensionDevelopmentPath: path.resolve(__dirname, ".."),
    extensionTestsPath: path.join(
      __dirname,
      process.env.IBCMD_TEST_SUITE || "host-suite.js",
    ),
    ...(executable
      ? { vscodeExecutablePath: executable }
      : { cachePath: path.join(root, "vscode-download") }),
    launchArgs: [
      workspaceFile,
      "--user-data-dir",
      path.join(run, "user-data"),
      "--extensions-dir",
      path.join(run, "extensions"),
      "--crash-reporter-directory",
      path.join(run, "crashes"),
      "--disable-workspace-trust",
      "--disable-updates",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-telemetry",
    ],
    extensionTestsEnv: {
      ...process.env,
      IBCMD_HOST_WORKSPACE: workspace,
      TMPDIR: process.env.TMPDIR || run,
    },
  });
})().catch((e) => {
  console.error(e.message);
  process.exitCode = 1;
});
