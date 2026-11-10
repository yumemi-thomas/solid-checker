import { createMemo, createSignal, untrack } from "solid-js";

// ADR 0937: lib.dom's global `addEventListener` and `dispatchEvent` are the window's own methods. The bare call registers a
// listener and dispatches synchronously as `window.*` does; Chrome warns STRICT_READ_UNTRACKED in each listener below.

// Uncertifiable (`listener-reach`): both calls bare (the fuzzer's `tbl-addeventlistener-sync-u-b` shape).
export function BareBoth() {
  const [r1] = createSignal(3);
  untrack(() => {
    const tf1 = () => {
      r1();
    };
    addEventListener("sc-evt", tf1);
  });
  dispatchEvent(new Event("sc-evt"));
  return <p />;
}

// A memo read, the same shape.
export function BareMemo() {
  const [s] = createSignal(3);
  const r1 = createMemo(() => s());
  untrack(() => {
    const tf1 = () => {
      r1();
    };
    addEventListener("sc-evt2", tf1);
  });
  dispatchEvent(new Event("sc-evt2"));
  return <p />;
}

// `window.addEventListener` delivered by the bare dispatch.
export function WindowListener() {
  const [r1] = createSignal(3);
  untrack(() => {
    const tf1 = () => {
      r1();
    };
    window.addEventListener("sc-evt3", tf1);
  });
  dispatchEvent(new Event("sc-evt3"));
  return <p />;
}
