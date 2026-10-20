import { createRoot, onCleanup } from "solid-js";

declare const thenable: PromiseLike<number>;

setTimeout(() => {
  onCleanup(() => {
    // module timer
  });
}, 0);

createRoot(() => {
  queueMicrotask(() => {
    onCleanup(() => {
      // root microtask
    });
    createRoot(() => {
      onCleanup(() => {
        // root in microtask
      });
    });
  });
});

function Panel() {
  Promise.resolve(1).then(() => {
    onCleanup(() => {
      // component then
    });
  });
  new MutationObserver(() => {
    onCleanup(() => {
      // component observer
    });
  });
  function tick() {
    onCleanup(() => {
      // named callback
    });
  }
  setInterval(tick, 1000);
  thenable.then(() => {
    onCleanup(() => {
      // thenable
    });
  });
  onCleanup(() => {
    // component body
  });
  return <div />;
}
