import { createMemo, createSignal, createStore, untrack } from "solid-js";
import { useHeld } from "./EscapedHook";

const [c] = createSignal(1);
const [st] = createStore({ x: 1 });
const unk = (): unknown => null;

function useThing() {
  return { read: (): number => 0, self: function (this: unknown) { return this; } };
}

// Each source escapes on one line and is read, or written, on another, in a
// component body: the read has its own row.
export function Carriers() {
  const m = new Map([["s", st]]);
  const s2 = (unk() as typeof st | null) ?? st;
  const o = Object.assign({}, { c });
  const cc = untrack(() => c);
  const fns = [{ run: (): number => 0 }];
  Object.assign(fns, [{ run: c }]);
  (globalThis as any).__escapedC = c;
  const x = useThing();
  const y: any = x.self();
  y[String("read")] = c;
  const held = useHeld();

  const fromMap = m.get("s")!.x;
  const fromNullish = s2.x;
  const fromAssign = o.c();
  const fromUntrack = cc();
  let fromEach = 0;
  fns.forEach((f) => { fromEach += f.run(); });
  const fromGlobal: number = (globalThis as any).__escapedC();
  const fromSelf = x.read();
  const fromHook = held.v;
  m.get("s")!.x = 5;
  return <p title={String(fromMap + fromNullish + fromAssign + fromUntrack + fromEach + fromGlobal + fromSelf + fromHook)} />;
}

// The same escapes, read where tracking is legal: no row at the reads.
export function Tracked() {
  const m = new Map([["s", st]]);
  const s2 = (unk() as typeof st | null) ?? st;
  const o = Object.assign({}, { c });
  const cc = untrack(() => c);
  const total = createMemo(() => m.get("s")!.x + s2.x + o.c() + cc());
  return <p title={String(total())} />;
}
