const { test } = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { resolveExecutable } = require("../out/executable");
test("machine overrides preserve paths and precedence", () => {
  assert.equal(
    resolveExecutable("/missing", "/custom path/tool", "legacy"),
    "/custom path/tool",
  );
  assert.equal(resolveExecutable("/missing", "", "legacy"), "legacy");
  assert.equal(resolveExecutable("/missing"), "ibcmd-rs");
});
test("Windows bundle precedes PATH", () => {
  const fs = require("node:fs"),
    old = fs.existsSync;
  try {
    fs.existsSync = (p) => p === path.join("root", "bin", "ibcmd-rs.exe");
    assert.equal(
      resolveExecutable("root", "", "", "win32"),
      path.join("root", "bin", "ibcmd-rs.exe"),
    );
  } finally {
    fs.existsSync = old;
  }
});
