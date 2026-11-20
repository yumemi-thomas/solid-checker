// solid-groove src/editor/DrumMachinePanel.tsx:257 (createControlGesture, src/commands/controlGesture.ts:21): callbacks in an
// options object handed to a project function, which keeps them in members of its result that the component calls later.
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
