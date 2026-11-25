import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: each export is the matching reactive-ir fixture `render-apply-named-write(-handed-twice, -deferred)`.
// rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE in `Named` and `HandedTwice` (its first apply), not in `Deferred`.

export function Named() {
  const [, setW] = createSignal(0);
  const apply = (v: number) => {
    setW(v);
  };
  createRenderEffect(() => 1, apply);
  return <div />;
}

export function HandedTwice() {
  const [, setW] = createSignal(0);
  const apply = () => {
    setW(1);
  };
  createRenderEffect(() => 1, apply);
  return <button onClick={apply} />;
}

export function Deferred() {
  const [, setW] = createSignal(0);
  const apply = (v: number) => {
    setW(v);
  };
  createRenderEffect(() => 1, apply, { defer: true });
  return <div />;
}
