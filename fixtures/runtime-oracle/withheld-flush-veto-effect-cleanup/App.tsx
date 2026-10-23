import { createEffect, createSignal, flush, onCleanup } from "solid-js";

// onCleanup in a createEffect apply. ADR 0362's flush veto withholds the
// proven-unowned apply program-wide because `Settle` flushes in an owned scope;
// these applies run from render's trailing flush and the scheduler, with no
// owner, so the dev build warns. Shapes from the corpus adjudication.

// error-menu-web ChangeList.tsx:97: the apply runs once the ref is set.
export function VirtualList() {
  const [scroller, setScroller] = createSignal<HTMLDivElement>();
  createEffect(
    () => scroller(),
    (element) => {
      if (element === undefined) return;
      const unmount = () => undefined;
      onCleanup(unmount);
    },
  );
  return <div ref={setScroller}>rows</div>;
}

// kui MessagesTab.tsx:218: a guarded apply that registers an observer.
export function InfiniteSentinel() {
  const [sentinel, setSentinel] = createSignal<HTMLDivElement>();
  createEffect(
    () => ({ view: "infinite", sentinel: sentinel() }),
    ({ view, sentinel: element }) => {
      if (view !== "infinite" || element === undefined || typeof IntersectionObserver === "undefined") return;
      const observer = new IntersectionObserver(() => undefined);
      observer.observe(element);
      onCleanup(() => observer.disconnect());
    },
  );
  return <div ref={setSentinel}>sentinel</div>;
}

// The veto: a flush in a component body (the corpus projects reach `flush` from
// test helpers and dependency source). It renders after the others, so nothing
// of theirs is queued when it runs.
export function Settle() {
  flush();
  return <span>settled</span>;
}
