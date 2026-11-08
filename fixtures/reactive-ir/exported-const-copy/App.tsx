import { createMemo } from "solid-js";
import { copied, copiedStore, plain } from "./source";

export function Reads() {
  const a = copied();
  const b = copiedStore.value;
  copiedStore.value = 2;
  return <p title={String(a + b + plain)} />;
}

export function Tracked() {
  const a = createMemo(() => copied() + copiedStore.value);
  return <p title={String(a())} />;
}
