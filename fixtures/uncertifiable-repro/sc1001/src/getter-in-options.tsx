// ai-memory-ui workspace-cascader.tsx:120-122: a getter in an options object
// handed to the app's own `createVirtualizer`, which merges and spreads it.
import { createEffect, createSignal, merge } from "solid-js";

type Options = { count: number; estimateSize: () => number };

function createVirtualizer(options: Options) {
  const resolved = merge({ overscan: 6 }, options);
  const [size, setSize] = createSignal(0);
  createEffect(
    () => ({ ...resolved }),
    (snapshot) => {
      setSize(snapshot.count * snapshot.estimateSize());
    },
  );
  return { size };
}

export function Cascader(props: { items: string[] }) {
  const list = createVirtualizer({
    get count() {
      return props.items.length;
    },
    estimateSize: () => 44,
  });
  return <div style={{ height: `${list.size()}px` }} />;
}
