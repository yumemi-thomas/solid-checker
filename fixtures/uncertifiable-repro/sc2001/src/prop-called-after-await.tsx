// inferay AgentContextPanel/index.tsx:47-62 (used at AgentChatView/index.tsx:95-98): the child's `close` awaits before it calls the
// `onClose` prop, and a keydown listener registered in an effect also runs it.
import { createEffect, createSignal, Show } from "solid-js";

function Panel(props: { onClose: () => void }) {
  const close = async () => {
    await Promise.resolve();
    props.onClose();
  };
  createEffect(
    () => [props.onClose],
    () => {
      const onKeyDown = (event: KeyboardEvent) => {
        if (event.key === "Escape") void close();
      };
      document.addEventListener("keydown", onKeyDown);
      return () => document.removeEventListener("keydown", onKeyDown);
    },
  );
  return <button onClick={() => void close()}>Save</button>;
}

export function Chat() {
  const [open, setOpen] = createSignal(true);
  return (
    <Show when={open()}>
      <Panel onClose={() => setOpen(false)} />
    </Show>
  );
}
