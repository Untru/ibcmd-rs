// Retained disposable fixture tests: no rm/rmdir or TempDir teardown.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const sync = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const { createHash } = require("node:crypto");
const hash = (b) => createHash("sha256").update(b).digest("hex");
async function harness({
  dirty = [],
  confirm,
  beforePrompt,
  afterGit,
  rpcError,
} = {}) {
  const root = await fs.mkdtemp(
    path.join(require("node:os").tmpdir(), "issue381-js-"),
  );
  const source = path.join(root, "source"),
    cache = path.join(root, "cache");
  await fs.mkdir(source);
  await fs.mkdir(cache);
  const git = (...args) =>
    execFileSync("git", ["-C", source, ...args], { encoding: "utf8" });
  git("init", "-q");
  git("config", "user.email", "fixture@example.invalid");
  git("config", "user.name", "fixture");
  await fs.mkdir(path.join(source, "CommonModules"));
  await fs.writeFile(path.join(source, "CommonModules/Test.xml"), "old");
  await fs.writeFile(path.join(source, "ConfigDumpInfo.xml"), "old info");
  await fs.writeFile(path.join(source, "README.md"), "unrelated");
  git("add", ".");
  git("commit", "-qm", "fixture");
  const warnings = [],
    errors = [],
    deleted = [],
    requests = [];
  const vscode = {
    ProgressLocation: { Notification: 1 },
    Uri: { file: (fsPath) => ({ scheme: "file", fsPath }) },
    workspace: {
      isTrusted: true,
      textDocuments: dirty.map((file) => ({
        isDirty: true,
        uri: { scheme: "file", fsPath: path.join(source, file) },
      })),
      getConfiguration: () => ({
        get: (key, fallback) =>
          ({ sourceDirectory: source, exportCacheDirectory: cache })[key] ??
          fallback,
      }),
      fs: {
        delete: async (uri, options) => {
          assert.equal(options.useTrash, true);
          deleted.push(uri.fsPath);
          await fs.rename(uri.fsPath, uri.fsPath + ".trash-fixture");
        },
      },
    },
    window: {
      withProgress: (_, run) =>
        run(
          {},
          {
            isCancellationRequested: false,
            onCancellationRequested: () => ({ dispose() {} }),
          },
        ),
      showInformationMessage() {},
      showWarningMessage: async (text) => {
        warnings.push(text);
        await beforePrompt?.(source);
        return confirm;
      },
    },
  };
  const module = { exports: {} };
  vm.runInNewContext(
    sync.readFileSync(path.join(__dirname, "../out/object-export.js"), "utf8"),
    {
      module,
      exports: module.exports,
      require: (id) =>
        id === "vscode"
          ? vscode
          : id === "./object-status"
            ? require("../out/object-status")
            : require(id),
      process,
      Buffer,
      AbortController,
    },
  );
  const rpc = {
    request: async (method, params) => {
      requests.push({ method, params });
      if (rpcError) throw rpcError;
      const files = ["CommonModules/Test.xml", "ConfigDumpInfo.xml"],
        expected = {};
      for (const file of files) {
        try {
          expected[file] = hash(await fs.readFile(path.join(source, file)));
        } catch (e) {
          if (e.code !== "ENOENT") throw e;
          expected[file] = null;
        }
      }
      await fs.mkdir(path.join(params.targetDir, "CommonModules"));
      await fs.writeFile(path.join(params.targetDir, files[0]), "new");
      await fs.writeFile(path.join(params.targetDir, files[1]), "new info");
      return { objects: ["CommonModule.Test"], files, removed: [], expected };
    },
  };
  const run = () =>
    module.exports.exportObjects(
      rpc,
      { uri: { fsPath: root } },
      "fixture",
      ["CommonModule.Test"],
      false,
      () => true,
    );
  return {
    root,
    source,
    cache,
    git,
    vscode,
    warnings,
    requests,
    run,
    ...module.exports,
  };
}
test("dirty buffer blocks publication, including ConfigDumpInfo", async () => {
  for (const file of ["CommonModules/Test.xml", "ConfigDumpInfo.xml"]) {
    const h = await harness({ dirty: [file] });
    await assert.rejects(h.run(), /несохранённые/);
    assert.equal(
      await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
      "old",
    );
    assert.equal(h.warnings.length, 0);
  }
});
test("Git modified files are listed and cancellation preserves all bytes", async () => {
  const h = await harness();
  await fs.writeFile(path.join(h.source, "CommonModules/Test.xml"), "local");
  assert.equal(await h.run(), false);
  assert.match(h.warnings[0], /CommonModules\/Test.xml/);
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "local",
  );
});
test("confirmed export updates exact targets and keeps originals and unrelated files", async () => {
  const h = await harness({ confirm: "Перезаписать перечисленные файлы" });
  await fs.writeFile(path.join(h.source, "CommonModules/Test.xml"), "local");
  assert.equal(await h.run(), true);
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "new",
  );
  assert.equal(
    await fs.readFile(path.join(h.source, "README.md"), "utf8"),
    "unrelated",
  );
  const stage = h.requests[0].params.targetDir;
  assert.equal(
    await fs.readFile(
      path.join(stage, "originals/CommonModules/Test.xml"),
      "utf8",
    ),
    "local",
  );
});
test("disk save while warning is open prevents overwrite", async () => {
  const h = await harness({
    confirm: "Перезаписать перечисленные файлы",
    beforePrompt: async (source) =>
      fs.writeFile(path.join(source, "CommonModules/Test.xml"), "newer"),
  });
  await fs.writeFile(path.join(h.source, "CommonModules/Test.xml"), "local");
  await assert.rejects(h.run(), /изменён во время/);
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "newer",
  );
});
test("untracked and ignored targets are Git conflicts", async () => {
  const h = await harness();
  await fs.writeFile(path.join(h.source, "untracked.xml"), "local");
  await fs.writeFile(path.join(h.source, ".gitignore"), "ignored.xml\n");
  await fs.writeFile(path.join(h.source, "ignored.xml"), "local");
  assert.deepEqual(
    await h.gitConflicts(
      h.source,
      ["untracked.xml", "ignored.xml", "README.md"],
      new AbortController().signal,
    ),
    ["untracked.xml", "ignored.xml"],
  );
});
test("nested source catches staged modifications even with diff.relative", async () => {
  const h = await harness();
  h.git("config", "diff.relative", "true");
  await fs.writeFile(path.join(h.source, "CommonModules/Test.xml"), "staged");
  h.git("add", "CommonModules/Test.xml");
  assert.deepEqual(
    await h.gitConflicts(
      path.join(h.source, "CommonModules"),
      ["Test.xml"],
      new AbortController().signal,
    ),
    ["Test.xml"],
  );
});
test("unrelated source symlinks do not block exact selected paths", async () => {
  const h = await harness();
  await fs.symlink(
    path.join(h.root, "missing"),
    path.join(h.source, "unrelated-link"),
  );
  assert.equal(await h.run(), true);
});
test("linked target is refused before overwrite", async () => {
  const h = await harness();
  await fs.rename(
    path.join(h.source, "CommonModules/Test.xml"),
    path.join(h.root, "original"),
  );
  await fs.symlink(
    path.join(h.root, "original"),
    path.join(h.source, "CommonModules/Test.xml"),
  );
  await assert.rejects(h.run(), /linked/);
  assert.equal(await fs.readFile(path.join(h.root, "original"), "utf8"), "old");
});
test("malformed plans cannot name traversal or conflicting writes", async () => {
  const h = await harness();
  assert.throws(
    () =>
      h.validatePlan({
        objects: [],
        files: ["../outside"],
        removed: [],
        expected: { "../outside": null },
      }),
    /Unsafe/,
  );
  assert.throws(
    () =>
      h.validatePlan({
        objects: [],
        files: ["x"],
        removed: ["x"],
        expected: { x: null },
      }),
    /Duplicate/,
  );
});

test("existing non-Git targets are listed as unversioned conflicts", async () => {
  const h = await harness();
  await fs.rename(
    path.join(h.source, ".git"),
    path.join(h.root, "retained-git"),
  );
  assert.equal(await h.run(), false);
  assert.match(h.warnings[0], /CommonModules\/Test.xml/);
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "old",
  );
});
test("case aliases cannot produce write/remove collisions", async () => {
  const h = await harness();
  const plan = {
    objects: [],
    files: ["CommonModules/test.xml"],
    removed: ["CommonModules/Test.xml"],
    expected: {},
  };
  assert.throws(() => h.uniqueTargets(h.source, plan, true), /alias/);
  assert.equal(
    h.targetIdentity("/SOURCE/Test.xml", true),
    h.targetIdentity("/source/test.xml", true),
  );
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "old",
  );
});
test("dirty casing aliases are refused on case insensitive source filesystems", async (t) => {
  const h = await harness();
  if (!(await h.insensitiveSource(h.source)))
    return t.skip("case sensitive filesystem");
  h.vscode.workspace.textDocuments.push({
    isDirty: true,
    uri: {
      scheme: "file",
      fsPath: path.join(h.source, "CommonModules/TEST.xml"),
    },
  });
  await assert.rejects(h.run(), /несохранённые/);
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "old",
  );
});
test("hard linked targets cannot modify an unrelated inode alias", async () => {
  const h = await harness();
  const unrelated = path.join(h.root, "unrelated.xml");
  await fs.link(path.join(h.source, "CommonModules/Test.xml"), unrelated);
  await assert.rejects(h.run(), /hard-linked/);
  assert.equal(await fs.readFile(unrelated, "utf8"), "old");
});
test("Git warnings recognize affected casing aliases on insensitive filesystems", async (t) => {
  const h = await harness();
  if (!(await h.insensitiveSource(h.source)))
    return t.skip("case sensitive filesystem");
  await fs.writeFile(path.join(h.source, "CommonModules/Test.xml"), "local");
  assert.deepEqual(
    await h.gitConflicts(
      h.source,
      ["COMMONMODULES/TEST.XML"],
      new AbortController().signal,
    ),
    ["COMMONMODULES/TEST.XML"],
  );
});

test("refused incomplete conversion cannot publish files or advance ConfigDumpInfo", async () => {
  const h = await harness({
    rpcError: new Error("selected source asset conversion is incomplete"),
  });
  await assert.rejects(
    h.run(),
    /selected source asset conversion is incomplete/,
  );
  assert.deepEqual(
    h.requests.map(({ method }) => method),
    ["objects/exportPlan"],
  );
  assert.equal(
    await fs.readFile(path.join(h.source, "CommonModules/Test.xml"), "utf8"),
    "old",
  );
  assert.equal(
    await fs.readFile(path.join(h.source, "ConfigDumpInfo.xml"), "utf8"),
    "old info",
  );
  assert.equal(
    await fs.readFile(path.join(h.source, "README.md"), "utf8"),
    "unrelated",
  );
});
