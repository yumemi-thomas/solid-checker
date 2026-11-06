// ADR 0751: `handed_value` follows six member steps to the value a caller supplied; past them the callee may be one,
// so a call after an await is an obligation, never clean.
import { createMemo } from "solid-js";

type Deep = { m0: { m1: { m2: { m3: { m4: { m5: { m6: { m7: { read: () => number } } } } } } } } };

export function Shallow(props: { r: { m0: { read: () => number } } }) {
  const m = createMemo(async () => { await Promise.resolve(); return props.r.m0.read(); });
  return <div>{m()}</div>;
}

export function Past(props: { r: Deep }) {
  const m = createMemo(async () => { await Promise.resolve(); return props.r.m0.m1.m2.m3.m4.m5.m6.m7.read(); });
  return <div>{m()}</div>;
}
