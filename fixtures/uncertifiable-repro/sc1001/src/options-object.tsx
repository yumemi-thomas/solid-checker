// ai-memory-ui workspace-cascader.tsx:120-122: callbacks in an options object
// handed to a project function, which keeps them in members of its result
// that only a package's virtualizer calls (here: nothing in the project).
import { createSignal } from "solid-js";

type ListOptions = { count: () => number; getScrollElement: () => HTMLElement | undefined; onRange: (first: number) => void };

function createVirtualList(options: ListOptions) {
  return { height: () => options.count() * 24, element: () => options.getScrollElement(), range: (first: number) => options.onRange(first) };
}

export function Cascader(props: { items: string[] }) {
  const [scroller, setScroller] = createSignal<HTMLElement>();
  const [first, setFirst] = createSignal(0);
  const list = createVirtualList({ count: () => props.items.length, getScrollElement: () => scroller(), onRange: (index) => setFirst(index) });
  return (
    <div ref={setScroller} style={{ height: `${list.height()}px` }}>
      {props.items[first()]}
    </div>
  );
}
