// donegeon-client ui/Picker.tsx:165 (used at ProjectPicker.tsx:63-72) and kui kernel StatusPill.tsx:84,88 (used at StatusPill.stories.tsx:62):
// a child tests a callback prop in a `Show` condition (`when={props.onAction}`) before calling it; StatusPill also reads `merge(props)`.
import { createSignal, merge, Show } from "solid-js";

function Picker(props: { actionLabel?: string; onAction?: () => void }) {
  return (
    <Show when={props.actionLabel && props.onAction}>
      <button onClick={() => props.onAction?.()}>{props.actionLabel}</button>
    </Show>
  );
}

function Pill(props: { tone?: "neutral" | "success"; children: string; onClick?: () => void }) {
  const p = merge({ tone: "neutral" } as const, props);
  return (
    <Show when={props.onClick !== undefined} fallback={<span class={p.tone}>{props.children}</span>}>
      <button class={p.tone} onClick={() => props.onClick?.()}>{props.children}</button>
    </Show>
  );
}

export function Page() {
  const [creating, setCreating] = createSignal(false);
  const [live, setLive] = createSignal(true);
  return (
    <>
      <Picker actionLabel="Create" onAction={() => setCreating(!creating())} />
      <Pill tone={live() ? "success" : "neutral"} onClick={() => setLive((v) => !v)}>
        {live() ? "LIVE" : "PAUSED"}
      </Pill>
    </>
  );
}
