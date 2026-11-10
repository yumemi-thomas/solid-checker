import { createMemo, createSignal } from "solid-js";

// ADR 0934: the body dispatches each element's event synchronously, so its
// handler runs in the component's strict-read window and owner (ADR 0356).

// Uncertifiable: a helper only an inline handler calls; its reads are the
// call's, unknown under the dispatch.
export function HelperCall() {
  const [page, setPage] = createSignal(0);
  const total = createMemo(() => 3);
  const goTo = (n: number) => {
    setPage(Math.min(n, total() - 1));
  };
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={() => goTo(page() + 1)} />;
  el.dispatchEvent(new Event("scroll"));
  return view;
}

// Uncertifiable: a module helper two calls deep.
const [shared, setShared] = createSignal(0);
const last = () => shared() - 1;
function jump() {
  setShared(last());
}
export function DeepHelper() {
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={() => jump()} />;
  el.dispatchEvent(new Event("scroll"));
  return view;
}

// Uncertifiable: a handler named in the attribute, its reads and its write.
export function NamedHandler() {
  const [page, setPage] = createSignal(0);
  const total = createMemo(() => 3);
  const onScroll = () => {
    setPage(Math.min(page() + 1, total() - 1));
  };
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={onScroll} />;
  el.dispatchEvent(new Event("scroll"));
  return view;
}
