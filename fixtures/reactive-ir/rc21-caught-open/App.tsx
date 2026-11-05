import { createMemo } from "solid-js";

const plain = { v: 1 };
const useRead = () => () => 1;

export function Caught(props: { read: () => number; value: number }) {
  const read = useRead();
  const held = {
    get v() {
      return props.value;
    },
  };
  const a = createMemo(() => {
    try {
      return props.read();
    } catch {
      return 0;
    }
  });
  const b = createMemo(() => {
    try {
      return props.value;
    } catch {
      return 0;
    }
  });
  const c = createMemo(() => {
    try {
      return read();
    } catch {
      return 0;
    }
  });
  const d = createMemo(() => {
    try {
      return held.v;
    } catch {
      return 0;
    }
  });
  const e = createMemo(() => {
    try {
      return plain.v;
    } catch {
      return 0;
    }
  });
  return <i>{a() + b() + c() + d() + e()}</i>;
}
