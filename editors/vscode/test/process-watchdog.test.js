const { test } = require("node:test");
const assert = require("node:assert/strict");
const { runBounded } = require("./process-watchdog");
test("watchdog preserves successful and failing child outcomes", async () => {
  assert.equal(
    await runBounded(process.execPath, ["-e", "process.exit(0)"], {
      timeout: 10_000,
    }),
    0,
  );
  assert.equal(
    await runBounded(process.execPath, ["-e", "process.exit(7)"], {
      timeout: 10_000,
    }),
    1,
  );
});
test("watchdog rejects an executable that cannot start", async () => {
  await assert.rejects(
    runBounded(process.execPath + ".missing", [], { timeout: 10_000 }),
    { code: "ENOENT" },
  );
});
test("watchdog terminates a synchronously blocked worker", async () => {
  const start = Date.now();
  assert.equal(
    await runBounded(process.execPath, ["-e", "while (true) {}"], {
      timeout: 500,
    }),
    1,
  );
  assert.ok(Date.now() - start < 15_000);
});

test("watchdog terminates worker descendants", async () => {
  const fs = require("node:fs/promises");
  const path = require("node:path");
  const os = require("node:os");
  const retained = await fs.mkdtemp(path.join(os.tmpdir(), "ibcmd-watchdog-"));
  const pidFile = path.join(retained, "descendant.pid");
  const script = `const {spawn}=require('node:child_process');
    const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'});
    require('node:fs').writeFileSync(process.argv[1],String(child.pid));
    while(true) {}`;
  assert.equal(
    await runBounded(process.execPath, ["-e", script, pidFile], {
      timeout: 3000,
    }),
    1,
  );
  const pid = Number(await fs.readFile(pidFile, "utf8"));
  // Allow the operating system to reap the killed descendant.
  let alive = true;
  for (let attempt = 0; attempt < 50 && alive; attempt++) {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
      alive = false;
    }
    if (alive) await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.equal(
    alive,
    false,
    `Descendant ${pid} survived; evidence retained at ${retained}`,
  );
});
