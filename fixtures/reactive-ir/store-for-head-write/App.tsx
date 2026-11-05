import { createStore } from "solid-js";

const [s] = createStore({ a: 1 as number | null, o: { a: 1 } });
const plain = { a: 1 };
const xs: number[] = JSON.parse("[]");
const keys: Record<string, number> = {};

export function Handlers() {
  return (
    <>
      {/* Violations: the loop iterates, so its head writes the store outside the setter. */}
      <button onClick={() => { for (s.a of [2]) void 0; }}>one</button>
      <button onClick={() => { for ({ a: s.a } of [{ a: 2 }]) void 0; }}>destructured</button>
      {/* Obligations: nothing proves the loop iterates (rc.21 writes only if it does). */}
      <button onClick={() => { for (s.a of [] as number[]) void 0; }}>empty</button>
      <button onClick={() => { for (s.a of xs) void 0; }}>binding</button>
      <button onClick={() => { for (s.o.a as any in keys) void 0; }}>keys</button>
      {/* Silent: a plain object. */}
      <button onClick={() => { for (plain.a of [2]) void 0; }}>plain</button>
    </>
  );
}
