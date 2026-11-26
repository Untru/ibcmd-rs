// Deliberately synthetic transport fixture; never evidence of a SQL login.
let input = Buffer.alloc(0);
function send(value) {
  const body = Buffer.from(JSON.stringify({ jsonrpc: "2.0", ...value }));
  const frame = Buffer.concat([
    Buffer.from(`Content-Length: ${body.length}\r\n\r\n`),
    body,
  ]);
  // Split in the middle of the header and UTF-8 payload to exercise byte buffering.
  process.stdout.write(frame.subarray(0, 7));
  process.stdout.write(frame.subarray(7));
}
process.stdin.on("data", (chunk) => {
  input = Buffer.concat([input, chunk]);
  while (true) {
    const h = input.indexOf("\r\n\r\n");
    if (h < 0) return;
    const n = Number(
      /Content-Length: (\d+)/.exec(input.subarray(0, h).toString())[1],
    );
    if (input.length < h + 4 + n) return;
    const m = JSON.parse(input.subarray(h + 4, h + 4 + n));
    input = input.subarray(h + 4 + n);
    if (m.method === "exit") process.exit(0);
    if (m.method === "crash") process.exit(7);
    if (m.method === "hang") continue;
    if (m.method === "$/cancelRequest") {
      send({
        id: m.params.id,
        error: { code: -32800, message: "secret must never be displayed" },
      });
      continue;
    }
    if (m.method === "badError") {
      send({
        id: m.id,
        error: { code: "secret-marker", message: "secret-marker" },
      });
      continue;
    }
    if (m.method === "error") {
      send({
        id: m.id,
        error: { code: -32014, message: "secret must never be displayed" },
      });
      continue;
    }
    if (m.method === "late") {
      setTimeout(() => send({ id: m.id, result: { late: true } }), 120);
      continue;
    }
    send({ id: m.id, result: m.params });
  }
});
