import { createEffect, createSignal, untrack } from "solid-js";
import { Virtualizer, elementScroll, observeElementOffset, observeElementRect, type VirtualizerOptions } from "@tanstack/virtual-core";

export default function App() {
  const [scroller] = createSignal<HTMLDivElement | null>(null);
  const options: VirtualizerOptions<HTMLDivElement, HTMLDivElement> = {
    count: 1,
    get getScrollElement() {
      const target = scroller();
      return () => target;
    },
    estimateSize: () => 44,
    scrollToFn: elementScroll,
    observeElementOffset,
    observeElementRect,
  };
  const instance = untrack(() => new Virtualizer(options));
  createEffect(() => ({ ...options }), (snapshot) => { instance.setOptions(snapshot); });
  return <p>{instance.options.count}</p>;
}
