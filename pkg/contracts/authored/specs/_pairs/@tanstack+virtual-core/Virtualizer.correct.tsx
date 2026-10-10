import { createEffect, createSignal, untrack } from "solid-js";
import { Virtualizer, elementScroll, observeElementOffset, observeElementRect, type VirtualizerOptions } from "@tanstack/virtual-core";

export default function App() {
  const [height] = createSignal(44);
  const options: VirtualizerOptions<HTMLDivElement, HTMLDivElement> = {
    count: 1,
    getScrollElement: () => null,
    get estimateSize() {
      const size = height();
      return () => size;
    },
    scrollToFn: elementScroll,
    observeElementOffset,
    observeElementRect,
  };
  const instance = untrack(() => new Virtualizer(options));
  createEffect(() => ({ ...options }), (snapshot) => { instance.setOptions(snapshot); });
  return <p>{instance.options.count}</p>;
}
