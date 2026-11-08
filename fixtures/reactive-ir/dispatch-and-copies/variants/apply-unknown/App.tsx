import { createMemo, createSignal } from "solid-js";

export function ApplyUnknown(props: { args: [number] }) {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    setV.apply(null, props.args);
    return 0;
  });
  return <p>{v()}</p>;
}
