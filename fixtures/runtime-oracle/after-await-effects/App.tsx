import { createEffect, createSignal, createTrackedEffect } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// The computation of an effect is a tracked async computation like a memo's.
export function EffectThen() {
  const [value] = createSignal(1);
  createEffect(
    () => pause().then(() => value()),
    () => {},
  );
  return <div />;
}

export function EffectAsyncMap() {
  const [value] = createSignal(1);
  createEffect(
    async () => {
      await pause();
      return [1].map(() => value())[0];
    },
    () => {},
  );
  return <div />;
}

// A tracked effect has no returned promise to attribute: its callback may not
// be async (`() => void | (() => void)`), and a handler it leaves behind is
// not on a flight.
export function TrackedEffectThen() {
  const [value] = createSignal(1);
  createTrackedEffect(() => {
    void pause().then(() => value());
  });
  return <div />;
}
