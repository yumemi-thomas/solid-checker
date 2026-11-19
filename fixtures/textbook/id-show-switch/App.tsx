import { Match, Show, Switch, createSignal } from "solid-js";

export function Status() {
  const [status, setStatus] = createSignal<"idle" | "busy" | "done">("idle");
  const [open, setOpen] = createSignal(false);
  return (
    <div>
      <Show when={open()} fallback={<button type="button" onClick={() => setOpen(true)}>open</button>}>
        <p>opened</p>
      </Show>
      <Switch fallback={<p>idle</p>}>
        <Match when={status() === "busy"}>
          <p>working</p>
        </Match>
        <Match when={status() === "done"}>
          <p>finished</p>
        </Match>
      </Switch>
      <button type="button" onClick={() => setStatus((value) => (value === "idle" ? "busy" : "done"))}>
        advance
      </button>
    </div>
  );
}
