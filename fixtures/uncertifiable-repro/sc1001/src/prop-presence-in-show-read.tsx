// kui feature-messages MessageFilterBar.tsx:699 (callback `props.onChange`, `props.live` read) onto kernel StatusPill.tsx:63,84,88: a child tests its
// callback prop in a `Show` condition (`when={props.onClick !== undefined}`) and reads `merge(props)`; the callback reads a component prop.
import { merge, Show } from "solid-js";

function Pill(props: { tone?: "neutral" | "success"; children: string; onClick?: () => void }) {
  const p = merge({ tone: "neutral" } as const, props);
  return (
    <Show when={props.onClick !== undefined} fallback={<span class={p.tone}>{props.children}</span>}>
      <button class={p.tone} onClick={() => props.onClick?.()}>{props.children}</button>
    </Show>
  );
}

export function Bar(props: { live: boolean; onChange: (live: boolean) => void }) {
  return (
    <Pill tone={props.live ? "success" : "neutral"} onClick={() => props.onChange(!props.live)}>
      {props.live ? "LIVE" : "PAUSED"}
    </Pill>
  );
}
