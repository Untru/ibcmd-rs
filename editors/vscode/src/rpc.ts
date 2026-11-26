import { spawn, ChildProcessWithoutNullStreams } from "node:child_process";

export function serverEnvironment(
  source: NodeJS.ProcessEnv,
  platform: NodeJS.Platform = process.platform,
): NodeJS.ProcessEnv {
  return Object.fromEntries(
    Object.entries(source).filter(([key]) => {
      const name = platform === "win32" ? key.toUpperCase() : key;
      return name !== "IBCMD_DB_PSW" && name !== "IBCMD_RS_CONFIG";
    }),
  );
}

export class RpcError extends Error {
  constructor(public code: number) {
    super(`ibcmd-rs request failed (${code})`);
  }
}
export class RpcClient {
  private child: ChildProcessWithoutNullStreams;
  private buffer: Buffer = Buffer.alloc(0);
  private sequence = 0;
  private dead = false;
  private disposing = false;
  private waiting = new Map<
    number,
    {
      resolve: (v: any) => void;
      reject: (e: Error) => void;
      timer: NodeJS.Timeout;
      cleanup: () => void;
    }
  >();
  constructor(
    executable: string,
    cwd: string,
    private onCrash: () => void = () => {},
    serverArgs: string[] = ["serve", "--stdio"],
  ) {
    // No shell, argv credentials, trace output, or server stderr/log forwarding.
    const env = serverEnvironment(process.env);
    this.child = spawn(executable, serverArgs, {
      cwd,
      env,
      stdio: "pipe",
      windowsHide: true,
    });
    this.child.stderr.resume();
    this.child.stdout.on("data", (chunk) => {
      try {
        this.receive(chunk);
      } catch {
        this.fail();
        this.child.kill();
      }
    });
    this.child.on("error", () => this.fail());
    this.child.on("exit", () => this.fail());
    this.child.stdin.on("error", () => this.fail());
  }
  private send(message: object) {
    if (this.dead) throw new Error("ibcmd-rs server is unavailable");
    const body = Buffer.from(JSON.stringify(message), "utf8");
    this.child.stdin.write(
      Buffer.concat([
        Buffer.from(`Content-Length: ${body.length}\r\n\r\n`),
        body,
      ]),
    );
  }
  request(
    method: string,
    params: object = {},
    signal?: AbortSignal,
    timeout = 30000,
  ): Promise<any> {
    if (signal?.aborted) return Promise.reject(new Error("Cancelled"));
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      const cancel = () => {
        if (!this.waiting.has(id)) return;
        try {
          this.send({
            jsonrpc: "2.0",
            method: "$/cancelRequest",
            params: { id },
          });
        } catch {}
        finish(new Error("Request cancelled or timed out"));
      };
      const cleanup = () => {
        signal?.removeEventListener("abort", cancel);
      };
      const finish = (e: Error) => {
        const p = this.waiting.get(id);
        if (p) {
          clearTimeout(p.timer);
          cleanup();
          this.waiting.delete(id);
          reject(e);
        }
      };
      const timer = setTimeout(cancel, timeout);
      this.waiting.set(id, { resolve, reject, timer, cleanup });
      signal?.addEventListener("abort", cancel, { once: true });
      try {
        this.send({ jsonrpc: "2.0", id, method, params });
      } catch {
        finish(new Error("Unable to send request"));
      }
    });
  }
  private receive(chunk: Buffer) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const end = this.buffer.indexOf("\r\n\r\n");
      if (end < 0) {
        if (this.buffer.length > 8192) throw new Error("Invalid header");
        return;
      }
      const header = this.buffer.subarray(0, end).toString("ascii");
      const lengths = [...header.matchAll(/^Content-Length:\s*(\d+)\s*$/gim)];
      if (lengths.length !== 1) throw new Error("Invalid length");
      const length = Number(lengths[0][1]);
      if (length > 64 * 1024 * 1024) throw new Error("Frame too large");
      if (this.buffer.length < end + 4 + length) return;
      const message = JSON.parse(
        this.buffer.subarray(end + 4, end + 4 + length).toString("utf8"),
      );
      this.buffer = this.buffer.subarray(end + 4 + length);
      if (message.jsonrpc !== "2.0") throw new Error("Invalid response");
      // Never print notifications or error text: either can contain server/user data.
      const p = this.waiting.get(message.id);
      if (!p) continue;
      clearTimeout(p.timer);
      p.cleanup();
      this.waiting.delete(message.id);
      if (message.error) {
        if (!Number.isInteger(message.error.code)) {
          p.reject(new Error("Invalid RPC error"));
          throw new Error("Invalid RPC error");
        }
        p.reject(new RpcError(message.error.code));
      } else p.resolve(message.result);
    }
  }
  private fail() {
    if (this.dead) return;
    this.dead = true;
    for (const p of this.waiting.values()) {
      clearTimeout(p.timer);
      p.cleanup();
      p.reject(new Error("ibcmd-rs server stopped"));
    }
    this.waiting.clear();
    // A broken input pipe can leave a live server with no usable transport.
    if (this.child.exitCode === null && !this.child.killed) this.child.kill();
    if (!this.disposing) this.onCrash();
  }
  async dispose() {
    if (this.dead || this.disposing) return;
    this.disposing = true;
    try {
      await this.request("shutdown", {}, undefined, 1000);
      this.send({ jsonrpc: "2.0", method: "exit" });
    } catch {}
    this.fail();
    this.child.stdin.end();
    if (this.child.exitCode === null) this.child.kill();
  }
}
