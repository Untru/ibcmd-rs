const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
const os = require("node:os");
const { execFileSync } = require("node:child_process");
const {
  classify,
  classifyVersioned,
  comparisons,
  localChanged,
  safePath,
} = require("../out/object-status");
const { gitBaseline } = require("../out/git-baseline");
const row = (status, changed = []) => ({
  object: "CommonModule.Test",
  version: "stable-version",
  status,
  changed,
  baseOnly: [],
  localOnly: [],
});
test("six directional states, including identical concurrent edits", () => {
  assert.equal(classify(row("same"), row("same"), false), "same");
  assert.equal(classify(row("modified"), row("modified"), false), "database");
  assert.equal(classify(row("modified"), row("same"), true), "file");
  assert.equal(classify(row("modified"), row("modified"), true), "both");
  assert.equal(classify(row("same"), row("modified"), true), "both");
  assert.equal(classify(row("notExported")), "databaseOnly");
  assert.equal(classify(row("notInBase")), "fileOnly");
  assert.equal(
    classify(row("same"), { ...row("same"), version: "different" }, false),
    "unknown",
  );
  assert.equal(
    classify({ ...row("same"), version: undefined }, row("same"), false),
    "unknown",
  );
  assert.equal(classify(row("same")), "unknown");
  assert.equal(classify(row("modified")), "unknown");
});
test("malformed or future RPC status never becomes clean", () => {
  assert.equal(
    classify(
      { ...row("same"), version: 1 },
      { ...row("same"), version: 1 },
      false,
    ),
    "unknown",
  );
  for (const value of [
    {},
    { objects: [{}] },
    { objects: [row("unknown")] },
    { objects: [row("same"), row("same")] },
  ])
    assert.throws(() => comparisons(value));
  for (const p of ["../secret", "/secret", "a/../../secret", "a\\b"])
    assert.throws(() => safePath("/tmp/source", p));
});
test("byte baseline handles binary changes, missing files and symlinks honestly", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "status-380-"));
  const a = path.join(root, "a"),
    b = path.join(root, "b");
  await fs.mkdir(a);
  await fs.mkdir(b);
  await fs.writeFile(path.join(a, "module"), Buffer.from([0, 255]));
  await fs.writeFile(path.join(b, "module"), Buffer.from([0, 255]));
  const current = row("modified", ["module"]),
    base = row("modified", ["module"]);
  assert.equal(await localChanged(a, b, current, base), false);
  await fs.writeFile(path.join(a, "module"), Buffer.from([1, 255]));
  assert.equal(await localChanged(a, b, current, base), true);
  assert.equal(await localChanged(a, b, row("same"), base), true);
  await fs.symlink(path.join(b, "module"), path.join(a, "link"));
  await assert.rejects(
    localChanged(a, b, row("modified", ["link"]), base),
    /Symlink/,
  );
});
test("Git baseline uses committed bytes and source subtree, rejects missing ref and cancellation", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "git-status-380-"));
  const repo = path.join(root, "repo"),
    cache = path.join(root, "cache");
  await fs.mkdir(repo);
  await fs.mkdir(path.join(repo, "src"));
  const git = (...args) =>
    execFileSync("git", ["-C", repo, ...args], { stdio: "pipe" });
  git("init");
  await fs.writeFile(path.join(repo, "src", "module"), "baseline");
  git("add", ".");
  git(
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.invalid",
    "commit",
    "-m",
    "baseline",
  );
  const commit = git("rev-parse", "HEAD").toString().trim();
  git(
    "update-index",
    "--add",
    "--cacheinfo",
    `160000,${commit},unrelated-submodule`,
  );
  git(
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.invalid",
    "commit",
    "-m",
    "unrelated submodule",
  );
  await fs.writeFile(path.join(repo, "src", "module"), "local");
  const snapshot = await gitBaseline(
    path.join(repo, "src"),
    "HEAD",
    cache,
    new AbortController().signal,
  );
  assert.equal(
    await fs.readFile(path.join(snapshot, "module"), "utf8"),
    "baseline",
  );
  await assert.rejects(
    gitBaseline(repo, "missing", cache, new AbortController().signal),
  );
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(gitBaseline(repo, "HEAD", cache, abort.signal));
});

test("Git baseline uses Git prefix through directory aliases and spaces", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "git-alias-380-"));
  const repo = path.join(root, "repo"),
    alias = path.join(root, "alias"),
    cache = path.join(root, "cache");
  await fs.mkdir(path.join(repo, "src with spaces"), { recursive: true });
  const git = (...args) => execFileSync("git", ["-C", repo, ...args]);
  git("init", "--quiet");
  await fs.writeFile(path.join(repo, "src with spaces", "module"), "committed");
  git("add", ".");
  git(
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.invalid",
    "commit",
    "--quiet",
    "-m",
    "baseline",
  );
  // A directory alias reproduces Git/Node path spelling differences on every OS.
  // Windows junctions need no symlink privilege, unlike directory symlinks.
  await fs.symlink(repo, alias, "junction");
  await fs.writeFile(path.join(repo, "src with spaces", "module"), "local");
  const snapshot = await gitBaseline(
    path.join(alias, "src with spaces"),
    "HEAD",
    cache,
    new AbortController().signal,
  );
  assert.equal(
    await fs.readFile(path.join(snapshot, "module"), "utf8"),
    "committed",
  );
  const bare = path.join(root, "bare");
  execFileSync("git", ["init", "--bare", "--quiet", bare]);
  await assert.rejects(
    gitBaseline(bare, "HEAD", cache, new AbortController().signal),
    /outside Git worktree/,
  );
});

function nativeState(
  current,
  reference,
  evidence,
  changed,
  baseline = "test-baseline",
) {
  return classifyVersioned(
    current,
    reference,
    evidence,
    changed,
    baseline,
    "test-snapshot",
  );
}
// Measured 8.5.1.1592 whole-import mismatch: metadata versions changed while
// Configuration/Language exported bytes stayed unchanged. No native parser is duplicated here.
test("version evidence corrects the observed Language/Configuration byte mismatch", () => {
  const language = {
    ...row("same"),
    object: "Language.English",
    version: "opaque-rpc-token",
  };
  const evidence = {
    basis: "native-config-version",
    baselineIdentity: "test-baseline",
    databaseSnapshot: "test-snapshot",
    object: language.object,
    objectVersion: language.version,
    conclusive: true,
    changed: true,
  };
  assert.equal(classify(language, language, false), "same"); // Existing byte preview, documented limit.
  assert.equal(nativeState(language, language, evidence, false), "database");
  assert.equal(nativeState(language, language, evidence, true), "both");
  const configuration = {
    ...row("modified", ["Configuration.xml"]),
    object: "Configuration",
  };
  const reference = { ...configuration, status: "same", changed: [] };
  assert.equal(classify(configuration, reference, true), "file");
  assert.equal(
    nativeState(
      configuration,
      reference,
      {
        ...evidence,
        object: "Configuration",
        objectVersion: configuration.version,
      },
      true,
    ),
    "both",
  );
});
test("native direction combines independent evidence into all six states", () => {
  const current = row("modified"),
    reference = row("modified");
  const e = {
    basis: "native-config-version",
    baselineIdentity: "test-baseline",
    databaseSnapshot: "test-snapshot",
    object: current.object,
    objectVersion: current.version,
    conclusive: true,
    changed: false,
  };
  for (const [db, file, expected] of [
    [false, false, "same"],
    [true, false, "database"],
    [false, true, "file"],
    [true, true, "both"],
  ])
    assert.equal(
      nativeState(current, reference, { ...e, changed: db }, file),
      expected,
    );
  assert.equal(nativeState(row("notExported")), "databaseOnly");
  assert.equal(nativeState(row("notInBase")), "fileOnly");
});
test("missing, partial, mixed-basis or stale native evidence never becomes clean", () => {
  const current = row("same"),
    reference = row("same");
  const e = {
    basis: "native-config-version",
    baselineIdentity: "test-baseline",
    databaseSnapshot: "test-snapshot",
    object: current.object,
    objectVersion: current.version,
    conclusive: true,
    changed: false,
  };
  for (const evidence of [
    undefined,
    { ...e, conclusive: false },
    { ...e, changed: undefined },
    { ...e, changed: 0 },
    { ...e, basis: "file-content-sha256" },
    { ...e, object: "Language.Other" },
    { ...e, objectVersion: "stale" },
    { ...e, baselineIdentity: "wrong-reference" },
  ])
    assert.equal(nativeState(current, reference, evidence, false), "unknown");
  assert.equal(
    nativeState(current, { ...reference, version: "stale" }, e, false),
    "unknown",
  );
  assert.equal(nativeState(current, undefined, e, false), "unknown");
  assert.equal(nativeState(current, reference, e, undefined), "unknown");
  assert.equal(
    nativeState(
      { ...current, version: 1 },
      { ...reference, version: 1 },
      { ...e, objectVersion: 1 },
      false,
    ),
    "unknown",
  );
});

test("version evidence is bound to exact reference object and baseline snapshot", () => {
  const current = row("same"),
    reference = row("same");
  const e = {
    basis: "native-config-version",
    baselineIdentity: "reference-A",
    databaseSnapshot: "test-snapshot",
    object: current.object,
    objectVersion: current.version,
    conclusive: true,
    changed: false,
  };
  assert.equal(
    classifyVersioned(
      current,
      reference,
      e,
      false,
      "reference-A",
      "test-snapshot",
    ),
    "same",
  );
  assert.equal(
    classifyVersioned(
      current,
      { ...reference, object: "Language.Other" },
      e,
      false,
      "reference-A",
      "test-snapshot",
    ),
    "unknown",
  );
  assert.equal(
    classifyVersioned(current, reference, e, false, "reference-B"),
    "unknown",
  );
  assert.equal(classifyVersioned(current, reference, e, false), "unknown");
});

test("native snapshot change between source/reference RPC reads is never clean", () => {
  const current = row("same");
  const e = {
    basis: "native-config-version",
    object: current.object,
    objectVersion: current.version,
    baselineIdentity: "baseline",
    databaseSnapshot: "snapshot-A",
    conclusive: true,
    changed: false,
  };
  assert.equal(
    classifyVersioned(current, current, e, false, "baseline", "snapshot-B"),
    "unknown",
  );
  assert.equal(
    classifyVersioned(current, current, e, false, "baseline"),
    "unknown",
  );
});
