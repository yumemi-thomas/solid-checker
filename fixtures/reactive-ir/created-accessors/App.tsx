import { children, createMemo, createSignal, mapArray, repeat } from "solid-js";

const [c] = createSignal(1);

export function Reads() {
  const kids = children(() => c());
  const rows = mapArray(() => [c()], (x) => x);
  const counted = repeat(() => c(), (i) => i);
  kids();
  rows();
  counted();
  createMemo(() => c())();
  return <p />;
}

export function Tracked() {
  const kids = children(() => c());
  const rows = mapArray(() => [c()], (x) => x);
  return <p>{[kids(), rows(), createMemo(() => c())()]}</p>;
}

export function Open() {
  const kids = children(() => c());
  kids.toArray();
  createMemo(async () => 1)();
  return <p />;
}
