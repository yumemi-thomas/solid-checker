// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json): reads
// in the body of a control-flow function child. rc.17 runs a non-keyed Show or
// Match child under untrack(..., "<Show>"/"<Match>") and a For mapper under
// setStrictRead("<For>") (solid-js dist/solid.dev.js:1588, 1635; @solidjs/signals
// dist/dev.js:3093-3099), so each read warns STRICT_READ_UNTRACKED.
import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";

// ai-memory-ui access.tsx:233 and 15 more, probus-hk Chrome.tsx:371: the Show
// callback reads its accessor in the body.
export function ShowSnapshot() {
  const [dialog] = createSignal<{ name: string } | null>({ name: "rotate" });
  const option = { Icon: (p: { size: number }) => `icon ${p.size}` };
  return (
    <section>
      <Show when={dialog()}>
        {(current) => {
          const d = current();
          return <p>{d.name}</p>;
        }}
      </Show>
      <Show when={option.Icon}>
        {(Icon) => Icon()({ size: 14 })}
      </Show>
    </section>
  );
}

// probus-hk Starred.tsx:298: the label computed in the callback body stays as
// it was while the condition stays truthy.
export function StaleLabel() {
  const [advice, setAdvice] = createSignal({ leaveIn: 5 });
  return (
    <section>
      <button type="button" data-probe="tick" onClick={() => setAdvice({ leaveIn: 4 })}>tick</button>
      <Show when={advice()}>
        {(a) => {
          const text = `${a().leaveIn} min`;
          return <span data-probe="label">{text}</span>;
        }}
      </Show>
    </section>
  );
}

// probus-hk Search.tsx:1296, ai-memory-ui graph.tsx:269: the For mapper reads the
// index accessor and a memo in its body.
export function ForMapperRead() {
  const [letters] = createSignal(["a", "b"]);
  const lookup = createMemo(() => new Map(letters().map((letter, at) => [letter, at])));
  return (
    <For each={letters()}>
      {(letter, index) => {
        const order = index();
        const at = lookup().get(letter);
        return <span>{order}{at}</span>;
      }}
    </For>
  );
}

// kui FeatureGate.tsx:121: the Match callback reads its accessor to pick the
// component it renders.
function Feature() {
  return <p>feature</p>;
}
export function MatchRead() {
  const [loaded] = createSignal({ component: Feature });
  return (
    <Switch>
      <Match when={loaded()}>
        {(module) => {
          const Loaded = module().component;
          return <Loaded />;
        }}
      </Match>
    </Switch>
  );
}
