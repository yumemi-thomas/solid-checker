import { createMemo, createSignal } from "solid-js";

const any: any = () => 1;
declare const options: { sync: boolean };
const named = () => 1;

export function Computes(props: { compute: () => unknown; flag: boolean }) {
  const [count] = createSignal(0);
  createMemo(async () => count(), { sync: true });
  createMemo(() => Promise.resolve(count()), { sync: true });
  createMemo(() => any(), { sync: true });
  createMemo(props.compute, { sync: true });
  createMemo(() => (props.flag ? Promise.resolve(1) : 1), { sync: true });
  createMemo(() => any(), options);
  createMemo(() => count(), { sync: true });
  createMemo(named, { sync: true });
  createMemo(() => count(), options);
  return <i />;
}
