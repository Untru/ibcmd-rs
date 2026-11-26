const { test } = require("node:test");
const assert = require("node:assert/strict");
const {
  runImport,
  importErrorMessage,
  ImportValidationError,
} = require("../out/import");
const request = {
  infobase: "lab",
  sourceDir: "/sources",
  files: ["Documents/Example/Ext/ObjectModule.bsl"],
};
const ready = {
  plan: "prepared",
  preflight: { ready: true, files: request.files, missingReferences: [] },
};
function harness(responses = [ready]) {
  const calls = [];
  const rpc = {
    request: async (method, params) => {
      calls.push({ method, params });
      return method === "objects/import/prepare"
        ? responses.shift()
        : { report: {} };
    },
  };
  return { calls, rpc };
}
test("only preflight runs when the final confirmation is declined", async () => {
  const { rpc, calls } = harness();
  assert.equal(
    await runImport(rpc, request, {
      current: async () => true,
      confirm: async () => false,
    }),
    undefined,
  );
  assert.deepEqual(
    calls.map((c) => c.method),
    ["objects/import/prepare"],
  );
});
test("connection change during confirmation refuses before publication", async () => {
  const { rpc, calls } = harness();
  let current = true;
  await assert.rejects(
    runImport(rpc, request, {
      current: async () => current,
      confirm: async () => {
        current = false;
        return true;
      },
    }),
    /Connection or settings changed/,
  );
  assert.equal(calls.length, 1);
});
test("cancellation during confirmation refuses before publication", async () => {
  const { rpc, calls } = harness();
  const abort = new AbortController();
  await assert.rejects(
    runImport(
      rpc,
      request,
      {
        current: async () => true,
        confirm: async () => {
          abort.abort();
          return true;
        },
      },
      abort.signal,
    ),
    /cancelled before publication/,
  );
  assert.equal(calls.length, 1);
});
test("dependency suggestion requires a separate explicit confirmation and new preflight", async () => {
  const missing = {
    plan: "unready",
    preflight: {
      ready: false,
      files: request.files,
      missingReferences: [
        {
          reference: "cfg:CatalogRef.Dependency",
          suggestedObject: "Catalog.Dependency",
        },
      ],
    },
  };
  const { rpc, calls } = harness([missing, ready]);
  let confirmations = 0;
  await runImport(rpc, request, {
    current: async () => true,
    confirm: async () => {
      confirmations++;
      return true;
    },
  });
  assert.equal(confirmations, 2);
  assert.deepEqual(
    calls.map((c) => c.method),
    ["objects/import/prepare", "objects/import/prepare", "objects/import"],
  );
  assert.deepEqual(calls[1].params.objects, [
    "Document.Example",
    "Catalog.Dependency",
  ]);
  assert.equal(calls[2].params.confirm, true);
});
test("declined dependency suggestion leaves the database unwritten", async () => {
  const { rpc, calls } = harness([
    {
      plan: "none",
      preflight: {
        ready: false,
        missingReferences: [
          {
            reference: "Catalog.Dependency",
            suggestedObject: "Catalog.Dependency",
          },
        ],
      },
    },
  ]);
  await runImport(rpc, request, {
    current: async () => true,
    confirm: async () => false,
  });
  assert.equal(calls.length, 1);
});
test("changed import preserves its selection and publishes the exact confirmed token", async () => {
  const { rpc, calls } = harness();
  await runImport(
    rpc,
    { infobase: "lab", sourceDir: "/sources", changed: true },
    { current: async () => true, confirm: async () => true },
  );
  assert.equal(calls[0].params.changed, true);
  assert.equal(calls[1].params.plan, "prepared");
});

test("changed dependency retry retains every original prepared file owner", async () => {
  const missing = {
    plan: "missing",
    preflight: {
      ready: false,
      files: request.files,
      missingReferences: [
        {
          reference: "Catalog.Dependency",
          suggestedObject: "Catalog.Dependency",
        },
      ],
    },
  };
  const { rpc, calls } = harness([missing, ready]);
  await runImport(
    rpc,
    { infobase: "lab", sourceDir: "/sources", changed: true },
    { current: async () => true, confirm: async () => true },
  );
  assert.deepEqual(calls[1].params.objects, [
    "Document.Example",
    "Catalog.Dependency",
  ]);
});

test("import UI never exposes server chains or unexpected error text", () => {
  const { RpcError } = require("../out/rpc");
  const sentinel =
    "private-user-sentinel /private/path-sentinel password-sentinel";
  const error = new RpcError(-32014, sentinel);
  error.detail = sentinel;
  assert.equal(importErrorMessage(error), "ibcmd-rs request failed (-32014)");
  assert.equal(importErrorMessage(new Error(sentinel)), "Import failed");
  assert.equal(
    importErrorMessage(new ImportValidationError("Save files first")),
    "Save files first",
  );
});

test("published recovery error remains explicit without exposing server paths", () => {
  const { RpcError } = require("../out/rpc");
  const error = new RpcError(-32016);
  error.detail = "/private/path-sentinel private-user-sentinel";
  assert.equal(
    importErrorMessage(error),
    "Database changes were published; backup update failed. Inspect recovery before retrying",
  );
  assert.ok(!importErrorMessage(error).includes("sentinel"));
});

test("dependency expansion covers every canonical root collection", async () => {
  const fs = require("node:fs"),
    path = require("node:path");
  const index = fs
    .readFileSync(
      path.join(
        __dirname,
        "../../../crates/ibcmd-metadata-model/src/metadata_model/index.rs",
      ),
      "utf8",
    )
    .split("pub const ROOT_COLLECTIONS:")[1]
    .split("];", 1)[0];
  const pairs = [...index.matchAll(/\("([^"\n]+)", "([^"\n]+)"\)/g)].map(
    (m) => [m[1], m[2]],
  );
  const websocket = fs.readFileSync(
    path.join(
      __dirname,
      "../../../crates/ibcmd-schema/src/websocket_client.rs",
    ),
    "utf8",
  );
  pairs.push([
    websocket.match(/FOLDER:.*?= "([^"]+)"/)[1],
    websocket.match(/KIND:.*?= "([^"]+)"/)[1],
  ]);
  assert.equal(pairs.length, 47);
  for (const [collection, kind] of pairs) {
    const files = [
      `${collection}/Example.xml`,
      `${collection}/Example/Ext/Module.bsl`,
    ];
    for (const changed of [false, true]) {
      const missing = {
        plan: "missing",
        preflight: {
          ready: false,
          files,
          missingReferences: [
            {
              reference: "Catalog.Dependency",
              suggestedObject: "Catalog.Dependency",
            },
          ],
        },
      };
      const { rpc, calls } = harness([missing, ready]);
      let confirmations = 0;
      await runImport(
        rpc,
        {
          infobase: "lab",
          sourceDir: "/sources",
          ...(changed ? { changed: true } : { files }),
        },
        {
          current: async () => true,
          confirm: async () => {
            confirmations++;
            return true;
          },
        },
      );
      assert.deepEqual(
        calls[1].params.objects,
        [`${kind}.Example`, "Catalog.Dependency"],
        `${collection}/${changed}`,
      );
      assert.equal(confirmations, 2);
      assert.equal(calls[2].method, "objects/import");
    }
  }
});
