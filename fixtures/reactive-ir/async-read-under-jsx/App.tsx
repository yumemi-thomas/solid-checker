import { Show, createEffect, createMemo, createSignal } from "solid-js";

// A cleanup written in JSX's function runs at dispose, no render: the source has settled by then.
export function CleanupInShow() {
  const a = createMemo(async () => 1);
  const [t, setT] = createSignal(1);
  return (
    <div>
      <Show when={t()}>
        {(_u) => {
          createEffect(() => 1, () => {
            return () => {
              a(); // no SC5003 violation
            };
          });
          return <div />;
        }}
      </Show>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}
