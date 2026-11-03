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

test("pre-existing cancellation makes no preparation or publication request", async () => {
  const { rpc, calls } = harness();
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(
    runImport(
      rpc,
      request,
      { current: async () => true, confirm: async () => true },
      abort.signal,
    ),
    ImportValidationError,
  );
  assert.deepEqual(calls, []);
});

test("cancellation while awaiting the final current check never publishes", async () => {
  const { rpc, calls } = harness();
  const abort = new AbortController();
  let checks = 0;
  await assert.rejects(
    runImport(
      rpc,
      request,
      {
        current: async () => {
          if (++checks === 3) abort.abort();
          return true;
        },
        confirm: async () => true,
      },
      abort.signal,
    ),
    ImportValidationError,
  );
  assert.deepEqual(
    calls.map((c) => c.method),
    ["objects/import/prepare"],
  );
});

test("prepare transport failure stays a normal failure without publication", async () => {
  const error = new Error("private-prepare-timeout");
  const calls = [];
  await assert.rejects(
    runImport(
      {
        request: async (method) => {
          calls.push(method);
          throw error;
        },
      },
      request,
      { current: async () => true, confirm: async () => true },
    ),
    (observed) => {
      assert.equal(observed, error);
      assert.equal(importErrorMessage(observed), "Import failed");
      return true;
    },
  );
  assert.deepEqual(calls, ["objects/import/prepare"]);
});

test("publish transport rejection is unconfirmed and never retried", async () => {
  for (const reason of ["Timeout", "AbortError", "server crash"]) {
    const calls = [];
    const error = new Error(`private-path/password ${reason}`);
    await assert.rejects(
      runImport(
        {
          request: async (method) => {
            calls.push(method);
            if (method === "objects/import/prepare") return ready;
            throw error;
          },
        },
        request,
        { current: async () => true, confirm: async () => true },
      ),
      (observed) => {
        assert.equal(observed.constructor.name, "ImportOutcomeUnknownError");
        assert.equal(
          importErrorMessage(observed),
          "Import outcome is unconfirmed; inspect the database before retrying",
        );
        assert.equal(observed.message.includes("private"), false);
        assert.equal(observed.cause, undefined);
        return true;
      },
    );
    assert.deepEqual(calls, ["objects/import/prepare", "objects/import"]);
  }
});

test("synchronous publication transport throw is also unconfirmed", async () => {
  let publications = 0;
  await assert.rejects(
    runImport(
      {
        request: (method) => {
          if (method === "objects/import/prepare")
            return Promise.resolve(ready);
          publications++;
          throw new Error("private synchronous transport failure");
        },
      },
      request,
      { current: async () => true, confirm: async () => true },
    ),
    (error) => {
      assert.equal(error.constructor.name, "ImportOutcomeUnknownError");
      return true;
    },
  );
  assert.equal(publications, 1);
});

test("acknowledged RPC cancellation and reconciliation codes retain their identity", async () => {
  const { RpcError } = require("../out/rpc");
  for (const [code, message] of [
    [-32800, "ibcmd-rs request failed (-32800)"],
    [
      -32017,
      "Import outcome is unconfirmed; inspect the database before retrying",
    ],
    [
      -32018,
      "Database changes were committed; inspect recovery before retrying",
    ],
    [
      -32016,
      "Database changes were published; backup update failed. Inspect recovery before retrying",
    ],
  ]) {
    const acknowledged = new RpcError(code);
    acknowledged.detail = "private-user/password/path";
    let publications = 0;
    await assert.rejects(
      runImport(
        {
          request: async (method) => {
            if (method === "objects/import/prepare") return ready;
            publications++;
            throw acknowledged;
          },
        },
        request,
        { current: async () => true, confirm: async () => true },
      ),
      (observed) => {
        assert.equal(observed, acknowledged);
        assert.equal(importErrorMessage(observed), message);
        assert.equal(importErrorMessage(observed).includes("private"), false);
        return true;
      },
    );
    assert.equal(publications, 1);
  }
});

test("acknowledged publication success survives late abort without replay", async () => {
  const abort = new AbortController();
  const result = { report: { state: "committed" } };
  const calls = [];
  const actual = await runImport(
    {
      request: async (method) => {
        calls.push(method);
        if (method === "objects/import/prepare") return ready;
        abort.abort();
        return result;
      },
    },
    request,
    { current: async () => true, confirm: async () => true },
    abort.signal,
  );
  assert.equal(actual, result);
  assert.deepEqual(calls, ["objects/import/prepare", "objects/import"]);
});
