// ADR 1074: `handed_value` follows twelve steps. A call through eight `const` aliases of a local literal is followed to the literal
// (no caller supplied it): the call after an await reads nothing a caller handed, so no obligation. Fourteen aliases pass the bound
// (an obligation, never clean), and the same eight aliases of a prop end at a value a caller supplied (an obligation).
import { createMemo } from "solid-js";

export function Within() {
  const r0 = { read: () => 1 };
  const r1 = r0;
  const r2 = r1;
  const r3 = r2;
  const r4 = r3;
  const r5 = r4;
  const r6 = r5;
  const r7 = r6;
  const r8 = r7;
  const m = createMemo(async () => {
    await Promise.resolve();
    return r8.read();
  });
  return <div>{m()}</div>;
}

export function PastTwelve() {
  const r0 = { read: () => 1 };
  const r1 = r0;
  const r2 = r1;
  const r3 = r2;
  const r4 = r3;
  const r5 = r4;
  const r6 = r5;
  const r7 = r6;
  const r8 = r7;
  const r9 = r8;
  const r10 = r9;
  const r11 = r10;
  const r12 = r11;
  const r13 = r12;
  const r14 = r13;
  const m = createMemo(async () => {
    await Promise.resolve();
    return r14.read();
  });
  return <div>{m()}</div>;
}

export function HandedRoot(props: { r: { read: () => number } }) {
  const r0 = props.r;
  const r1 = r0;
  const r2 = r1;
  const r3 = r2;
  const r4 = r3;
  const r5 = r4;
  const r6 = r5;
  const r7 = r6;
  const r8 = r7;
  const m = createMemo(async () => {
    await Promise.resolve();
    return r8.read();
  });
  return <div>{m()}</div>;
}
