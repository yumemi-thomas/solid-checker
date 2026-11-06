import { createMemo, For, resolve } from "solid-js";

// ADR 0834: an IIFE in a JSX expression of a `For` row runs in that
// expression's insert, a tracked computation, not in a nested handler.

// Proved: `resolve` there throws (rc.21 in Chrome: "Cannot call resolve inside
// a reactive scope" in `<For> › div.children`): an SC2004 violation.
export function ResolveInRow() {
  return <For each={[1]}>{() => <div>{(() => { void resolve(() => 1); return 1; })()}</div>}</For>;
}

// Uncertifiable: a read of an async memo through a helper chain the IIFE runs
// (W's `for` wrapper chain-5, certified before): ASYNC_OUTSIDE_LOADING_BOUNDARY
// in Chrome; whether the source is pending there is SC5003's open question.
export function PendingInRow() {
  const pending = createMemo(async () => 1);
  const wrapUse = () => { pending(); };
  return (
    <For each={[1]}>
      {() => (
        <p>
          {(() => {
            const hc0 = () => { wrapUse(); };
            const hc1 = () => { hc0(); };
            const hc2 = () => { hc1(); };
            const hc3 = () => { hc2(); };
            const hc4 = () => { hc3(); };
            hc4();
            return 1;
          })()}
        </p>
      )}
    </For>
  );
}

// Silent: a handler in the row runs when the event fires, with no observer.
export function ResolveInRowHandler() {
  return <For each={[1]}>{() => <button onClick={() => { void resolve(() => 1); }}>x</button>}</For>;
}
