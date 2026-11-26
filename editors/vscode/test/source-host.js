const path = require("node:path");
const fs = require("node:fs/promises");
const { runTests } = require("@vscode/test-electron");
(async () => {
  delete process.env.ELECTRON_RUN_AS_NODE;
  const root = process.env.IBCMD_TEST_ROOT;
  if (!root || !process.env.IBCMD_TEST_BINARY)
    throw new Error("Укажите IBCMD_TEST_ROOT и IBCMD_TEST_BINARY");
  await fs.mkdir(root, { recursive: true });
  const dir = await fs.mkdtemp(path.join(root, "source-host-"));
  const workspace = path.join(dir, "workspace");
  await fs.mkdir(workspace);
  await runTests({
    extensionDevelopmentPath: path.resolve(__dirname, ".."),
    extensionTestsPath: path.join(__dirname, "source-host-suite.js"),
    ...(process.env.VSCODE_EXECUTABLE_PATH
      ? { vscodeExecutablePath: process.env.VSCODE_EXECUTABLE_PATH }
      : { cachePath: path.join(root, "vscode-download") }),
    launchArgs: [
      workspace,
      "--user-data-dir",
      path.join(dir, "user"),
      "--extensions-dir",
      path.join(dir, "extensions"),
      "--disable-workspace-trust",
      "--disable-updates",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-telemetry",
    ],
  });
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
