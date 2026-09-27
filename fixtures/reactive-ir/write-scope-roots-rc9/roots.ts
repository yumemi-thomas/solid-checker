import {
  action,
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  createStore,
  flush,
  onSettled,
  untrack,
} from "solid-js";

const [count, setCount] = createSignal(0);
const [state, setState] = createStore({ value: 0 });
const save = action(function* () {});

// 1. `flush(fn)` runs `fn` inline under the caller's owner, so a write in it
// is exactly as legal as at the `flush` call.
const flushWrite = () => setCount(2);
export const flushed = createMemo(() => {
  // SC2001 on every release: the memo compute's owner, inline and by name.
  flush(() => setCount(3));
  flush(flushWrite);
  return count();
});
createRoot(() => {
  // SC2001 on every release: the root body's owner.
  flush(() => setCount(4));
});
// None: module scope and an event listener have no owner.
flush(() => setCount(5));
flush(flushWrite);
document.addEventListener("click", () => flush(() => setCount(6)));

// 2. An action call throws under a root owner on every release: SC2002.
createRoot(() => {
  save();
});
createRoot((dispose) => {
  untrack(() => save());
  return dispose;
});
createRoot(() => {
  // A memo compute nested in a root is the memo's scope: SC2002.
  createMemo(() => {
    save();
    return count();
  });
  // None: an effect apply has no owner, `onSettled` is a leaf, and an event
  // listener runs from the event loop.
  createEffect(
    () => count(),
    () => {
      save();
    },
  );
  onSettled(() => {
    save();
  });
  document.addEventListener("click", () => {
    save();
  });
});
save();

// 3. A same-file function passed to `createRoot` by name is the root body.
function init() {
  // SC2001 and SC2002 on every release.
  setCount(7);
  save();
  // SC2001 only where the store setter's guard rejects roots (rc.9).
  setState((s) => {
    s.value = 5;
  });
}
createRoot(init);
const initWithDispose = (dispose: () => void) => {
  // SC2001 on every release.
  setCount(8);
  return dispose;
};
createRoot(initWithDispose);

// None: `mount`'s parameter shadows `notARootBody`, so the root body is
// whatever `mount` is given, never the same-named module function. Resolving
// by name would report the write below.
function notARootBody() {
  setCount(9);
}
function mount(notARootBody: () => void) {
  createRoot(notARootBody);
}
mount(() => {});
document.addEventListener("click", notARootBody);

export { state };
