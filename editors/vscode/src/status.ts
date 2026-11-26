export interface Base {
  id: string;
  name: string;
  server?: string;
  rowsDir?: string;
  platform: string;
  source: string;
  mask: boolean;
}
export interface Metadata {
  label?: string;
  auth?: "windows" | "sql";
  user?: string;
}
export function pendingText(value: any): string {
  if (value?.conclusive !== true || typeof value.pending !== "boolean")
    return "pending unknown";
  return value.pending ? "pending changes" : "no pending changes";
}
// A generation belongs to one selection/reload; late answers cannot replace the current status.
export class Generation {
  private value = 0;
  next() {
    return ++this.value;
  }
  capture() {
    return this.value;
  }
  current(value: number) {
    return this.value === value;
  }
}
