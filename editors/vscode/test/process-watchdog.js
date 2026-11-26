const { spawn, execFile } = require("node:child_process");
// Separate supervisor: synchronous installer/CLI waits cannot block its deadline.
exports.runBounded = (executable, args, { timeout }) =>
  new Promise((resolve, reject) => {
    const child = spawn(executable, args, {
      stdio: "inherit",
      detached: process.platform !== "win32",
    });
    let expired = false;
    const timer = setTimeout(() => {
      expired = true;
      console.error(
        `Installed acceptance exceeded ${timeout} ms; terminating process tree ${child.pid}`,
      );
      if (process.platform === "win32") {
        execFile(
          "taskkill",
          ["/PID", String(child.pid), "/T", "/F"],
          { timeout: 10_000 },
          (error) => {
            if (error) console.error(error);
            child.kill("SIGKILL");
            resolve(1);
          },
        );
      } else {
        try {
          process.kill(-child.pid, "SIGKILL");
        } catch (error) {
          if (error.code !== "ESRCH") console.error(error);
        }
        resolve(1);
      }
    }, timeout);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    // Exit, not close: descendants can retain inherited Windows stdio handles.
    child.once("exit", (code) => {
      if (!expired) {
        clearTimeout(timer);
        resolve(code === 0 ? 0 : 1);
      }
    });
  });
