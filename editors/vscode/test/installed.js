const path = require("node:path");
const { runBounded } = require("./process-watchdog");
runBounded(process.execPath, [path.join(__dirname, "installed-run.js")], {
  timeout: 8 * 60_000,
})
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
