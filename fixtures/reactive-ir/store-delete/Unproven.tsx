import { createStore } from "solid-js";

type S = {
  a?: number;
  b: number;
  nick: string | undefined;
  u: unknown;
  rec: Record<string, number>;
  items: number[];
  t: [number];
  o: { c?: number };
};
const [st] = createStore<S>({ b: 1, nick: "n", u: 1, rec: { k: 1 }, items: [1], t: [1], o: { c: 1 } });

// ADR 0829: tsc accepts each (strict, no exactOptionalPropertyTypes) and the
// proxy warns STORE_WRITE_OUTSIDE_SETTER for each in Chrome (rc.21), but the
// producer proves the operand neither optional and writable nor rejected:
// uncertifiable (`delete-operand-unproven`), never silent.
export function Unproven(key: string) {
  delete st["a"];
  delete st.rec[key];
  delete st.rec.k;
  delete st.items[0];
  delete st.o["c"];
  delete st.nick;
  delete st.u;
  const kk = "a" as const;
  delete st[kk];
}

// Silent: TS2790 (required operands) on the published typings, a static key,
// a literal computed key and a tuple element alike: TypeScript's (the
// producer's rejection proof, ADR 0829).
export function RejectedKeys() {
  delete st.b;
  delete st["b"];
  delete st.t[0];
}
