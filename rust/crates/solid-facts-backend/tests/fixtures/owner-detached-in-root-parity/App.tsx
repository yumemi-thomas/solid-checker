import { createEffect, createReaction, createRoot, createSignal, onCleanup, runWithOwner } from "solid-js";

const [count] = createSignal(0);

createRoot(() => {
  createEffect(
    () => count(),
    () => {
      onCleanup(() => {
        // effect apply
      });
      createRoot(() => {
        onCleanup(() => {
          // root in apply
        });
      });
    },
  );
  const track = createReaction(() => {
    onCleanup(() => {
      // reaction
    });
  });
  track(() => count());
  runWithOwner(null, () => {
    onCleanup(() => {
      // null owner
    });
  });
  return (
    <button
      onClick={() => {
        onCleanup(() => {
          // handler
        });
      }}
    />
  );
});
