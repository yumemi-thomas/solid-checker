import { createEffect, createMemo, createSignal } from "solid-js";

// The census: the program puts its own function at these members.
const own: any = {};
own.slice = (a: number, b?: number) => own;
own.trim = () => own;
own.forEach = (cb: (v: string) => void) => queueMicrotask(() => cb("x"));
own.flatMap = (cb: (v: string) => unknown[]) => [];

// Dedup: `trim` on the withheld `slice`'s result keeps its own row, beside the one at `target.slice`.
export function scopeOf(raw: string): string {
  const target = raw.trim();
  const colon = target.indexOf(":");
  return colon > 0 ? target.slice(0, colon).trim() : target;
}

// Owner: `watch` runs from the body (owned) and from a withheld `forEach`'s callback, which an impostor may run with no owner.
export function watch(expr: () => string) {
  createEffect(expr, () => {});
}
export const names: string[] = (globalThis as any).names;

// Pending: a pending read in a withheld `flatMap`'s callback inside a memo keeps the row the standard run would place (SC5003).
export function App(props: { raw: string }) {
  const [s] = createSignal("a");
  const db = createMemo(async () => 1);
  watch(s);
  names.forEach((name) => watch(() => name));
  const crumbs = createMemo(() => names.flatMap((n) => [n, db()]));
  return <p title={scopeOf(props.raw)}>{crumbs().length}</p>;
}
