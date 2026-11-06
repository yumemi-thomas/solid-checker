import { createSignal } from "solid-js";

const [label] = createSignal("a");
const [on] = createSignal(false);
const [count] = createSignal(0);
const table: Record<string, number> = { a: 1 };

// ADR 0830: SC1005's positions TypeScript closes (TS2322, TS2538, TS2365),
// each on a suppressed line: nothing reports them, uncertifiable.
export function Suppressed() {
  // @ts-ignore
  const key = table[label];
  // @ts-ignore
  const sum = count + 1;
  return (
    <div
      // @ts-ignore
      title={label}
      // @ts-ignore
      class={{ active: on }}
    >
      {key}
      {sum}
    </div>
  );
}

// The directive covers another line: TS2538 stands, the position is TypeScript's.
export function Elsewhere() {
  // @ts-ignore
  const unused: number = "a";
  const key = table[label];
  return <div>{key}{unused}</div>;
}
