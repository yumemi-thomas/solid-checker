import { createEffect, createSignal, untrack } from "solid-js";
import { Virtualizer, elementScroll, observeElementOffset, observeElementRect, type VirtualizerOptions } from "@tanstack/virtual-core";

export default function App() {
  const [margin] = createSignal(12);
  const options: VirtualizerOptions<HTMLDivElement, HTMLDivElement> = {
    count: 1,
    getScrollElement: () => null,
    estimateSize: () => 44,
    get scrollMargin() { return margin(); },
    scrollToFn: elementScroll,
    observeElementOffset,
    observeElementRect,
  };
  const instance = new Virtualizer(options);
  createEffect(() => ({ ...options }), (snapshot) => { instance.setOptions(snapshot); });
  return <p>{instance.options.scrollMargin}</p>;
}
