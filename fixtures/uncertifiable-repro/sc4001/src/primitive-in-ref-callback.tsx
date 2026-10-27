// Violation. Corpus: probus-hk src/routes/Search.tsx:586 (createEffect), components/Layout.tsx:73,
// RailDiagram.tsx:211 (onCleanup; 4 more like them): a ref callback runs after the owner is gone,
// so an effect or cleanup created there has no owner.
import { createEffect, createSignal, onCleanup } from "solid-js";

export function Search() {
  const [db] = createSignal(0);
  return (
    <input
      ref={(el: HTMLInputElement) => {
        createEffect(
          () => db(),
          () => el.focus(),
        );
        const observer = new ResizeObserver(() => el.blur());
        observer.observe(el);
        onCleanup(() => observer.disconnect());
      }}
    />
  );
}
