import { createEffect, createSignal, untrack } from "solid-js";
import { Virtualizer, elementScroll, observeElementOffset, observeElementRect, type VirtualizerOptions } from "@tanstack/virtual-core";

export default function App() {
  const [activeProjects] = createSignal(["project"]);
  const options: VirtualizerOptions<HTMLDivElement, HTMLDivElement> = {
    get count() { return activeProjects().length; },
    getScrollElement: () => null,
    estimateSize: () => 44,
    scrollToFn: elementScroll,
    observeElementOffset,
    observeElementRect,
  };
  const instance = untrack(() => new Virtualizer(options));
  createEffect(() => ({ ...options }), (snapshot) => { instance.setOptions(snapshot); });
  return <p>{instance.options.count}</p>;
}
