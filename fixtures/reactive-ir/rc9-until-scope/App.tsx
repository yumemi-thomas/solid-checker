// SC2005 until-in-tracked-scope. `until` is new in @solidjs/signals@2.0.0-rc.9
// and opens with resolve's observer guard (`dist/dev.js:2718-2722`):
//
//   if (getObserver()) throw new Error("Cannot call until inside a reactive scope; …")
//
// so it throws inside tracked computes and tracked JSX and is legal wherever
// no observer is active. The production bundle has no guard
// (`dist/prod/signals.js:530`); the rule mirrors the dev throw, as SC2004 does
// for resolve.
import { action, createEffect, createMemo, createSignal, createTrackedEffect, until, untrack } from "solid-js";

const [ready, setReady] = createSignal(false);

// Positive: a memo compute is an active observer.
export function InMemoCompute() {
  const label = createMemo(() => {
    void until(() => ready());
    return ready();
  });
  return <div>{String(label())}</div>;
}

// Positive: an effect's compute (argument 0) is an active observer.
export function InEffectCompute() {
  createEffect(
    () => until(() => ready()),
    () => {}
  );
  return <div />;
}

// Positive: createTrackedEffect's callback is tracked.
export function InTrackedEffect() {
  createTrackedEffect(() => {
    void until(() => ready());
  });
  return <div />;
}

// Positive: tracked JSX runs in a render-effect compute.
export function InTrackedJsx() {
  return <div>{String(until(() => ready()))}</div>;
}

// Negative: untrack clears the observer the guard reads, even inside a memo.
export function InUntrackWithinMemo() {
  const label = createMemo(() => {
    untrack(() => void until(() => ready()));
    return ready();
  });
  return <div>{String(label())}</div>;
}

// Negative: the component body runs with no observer.
export function InComponentBody() {
  const pending = until(() => ready(), { timeout: 1000 });
  void pending;
  return <div />;
}

// Negative: an event handler is imperative code.
export function InEventHandler() {
  return (
    <button
      onClick={async () => {
        await until(() => ready());
        setReady(false);
      }}
    >
      wait
    </button>
  );
}

// Negative: an effect's apply function runs untracked.
export function InEffectApply() {
  createEffect(
    () => ready(),
    value => {
      void until(() => value);
    }
  );
  return <div />;
}

// Negative: the use rc.9's own documentation gives, a step of an action.
export const confirm = action(function* () {
  yield;
  yield until(() => ready(), { timeout: 10_000 });
});

// Negative: module scope has no observer.
export const initial = until(() => ready());
