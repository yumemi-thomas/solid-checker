// SC2004 resolve-in-tracked-scope:  the rc.0 dev guard is `getObserver()`
// (probed) — resolve() throws inside tracked computes and tracked JSX, and is
// legal in untrack, component bodies, event handlers, apply callbacks, and
// module scope. Every positive and negative here pins one probed cell.
import { createEffect, createMemo, createSignal, createTrackedEffect, resolve, untrack } from "solid-js";

const [user, setUser] = createSignal("ada");

// Tracked memo compute: throws in dev (probed). The promise is discarded so
// the memo stays synchronous and only SC2004 owns this defect.
export function InMemoCompute() {
  const label = createMemo(() => {
    void resolve(() => user());
    return user();
  });
  return <div>{String(label())}</div>;
}

// Tracked effect compute (argument 0 of createEffect): throws in dev.
export function InEffectCompute() {
  createEffect(
    () => resolve(() => user()),
    () => {}
  );
  return <div />;
}

// createTrackedEffect callback: tracked leaf — throws in dev (probed).
export function InTrackedEffect() {
  createTrackedEffect(() => {
    void resolve(() => user());
  });
  return <div />;
}

// Tracked JSX expression: JSX runs in render-effect computes, so the observer
// is active and the dev guard throws.
export function InTrackedJsx() {
  return <div>{resolve(() => user())}</div>;
}

// untrack clears the observer — legal even inside a memo (probed; the RFC's
// broader "reactive scope" wording is narrower at runtime).
export function InUntrackWithinMemo() {
  const label = createMemo(() => {
    untrack(() => void resolve(() => user()));
    return user();
  });
  return <div>{String(label())}</div>;
}

// Component body: devComponent runs the body untracked (probed: getObserver()
// is null there) — legal.
export function InComponentBody() {
  const pending = resolve(() => user());
  void pending;
  return <div />;
}

// Event handler: imperative code is exactly where resolve() belongs.
export function InEventHandler() {
  return (
    <button
      onClick={async () => {
        const current = await resolve(() => user());
        setUser(current);
      }}
    >
      export
    </button>
  );
}

// Effect apply callback (argument 1): runs untracked — legal.
export function InEffectApply() {
  createEffect(
    () => user(),
    value => {
      void resolve(() => value);
    }
  );
  return <div />;
}

// Module scope: no observer exists before any computation runs — legal.
export const initial = resolve(() => user());
