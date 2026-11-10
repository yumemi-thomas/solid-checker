import { createMemo, createSignal } from "solid-js";

// ADR 0934, the twin of `event-dispatch-handler-reads`: nothing dispatches, so
// every handler runs on its event, outside any window or owner.

// Clean: a helper only an inline handler calls.
export function HelperCall() {
  const [page, setPage] = createSignal(0);
  const total = createMemo(() => 3);
  const goTo = (n: number) => {
    setPage(Math.min(n, total() - 1));
  };
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={() => goTo(page() + 1)} />;
  return view;
}

// Clean: a module helper two calls deep.
const [shared, setShared] = createSignal(0);
const last = () => shared() - 1;
function jump() {
  setShared(last());
}
export function DeepHelper() {
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={() => jump()} />;
  return view;
}

// Clean: a handler named in the attribute, its reads and its write.
export function NamedHandler() {
  const [page, setPage] = createSignal(0);
  const total = createMemo(() => 3);
  const onScroll = () => {
    setPage(Math.min(page() + 1, total() - 1));
  };
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={onScroll} />;
  return view;
}
