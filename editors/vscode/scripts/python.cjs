const { spawnSync } = require("node:child_process");
const executable =
  process.env.PYTHON || (process.platform === "win32" ? "python" : "python3");
const result = spawnSync(executable, process.argv.slice(2), {
  stdio: "inherit",
});
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
