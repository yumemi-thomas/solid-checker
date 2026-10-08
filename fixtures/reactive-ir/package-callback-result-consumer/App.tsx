import { createSignal } from "solid-js";
import { takeResult, getResult, getTrackedResult, callTrackedResult, unknownResult, escapeResult, readResult, iterateResult, coerceResult, escapingTrackedResult, noResultUses } from "reactive-package";

export function PrimitiveNegative() {
  takeResult(() => 1); readResult(() => 1); coerceResult(() => "x"); iterateResult(() => "x");
  getResult(() => ({ value: 1 }));
  getResult(() => { return { value: 1 }; });
  takeResult((() => 1) satisfies (() => number));
  noResultUses(() => unknown);
  return <div />;
}

export function ProducerViolation() {
  const [count, setCount] = createSignal(0);
  takeResult(() => { setCount(1); return 1; });
  return <p>{count()}</p>;
}

export function ExactReturnedArrowViolation() {
  const [count, setCount] = createSignal(0);
  callTrackedResult(() => () => { setCount(1); return count(); });
  return <p>{count()}</p>;
}

export function ExactReturnedArrowClean() {
  const [count] = createSignal(0);
  callTrackedResult(() => () => count());
  return <p>{count()}</p>;
}

declare const unknown: unknown;
declare const proxy: object;
declare function outside<T>(x: T): void;
export function OpenResults() {
  takeResult(() => unknown);
  getResult(() => proxy);
  getResult(() => ({ get value() { return unknown; } }));
  getResult(() => ({ ...proxy }));
  getTrackedResult(() => ({ get value() { return unknown; } }));
  unknownResult(() => 1);
  outside(escapeResult(() => () => 1));
  outside(escapingTrackedResult(() => () => 1));
  const f = () => 1;
  callTrackedResult(() => f);
  takeResult(() => () => 1);
  takeResult(() => (() => 1) as (() => number));
  return <div />;
}

// Returning through a project wrapper cannot transfer an authored result-use
// census into a locally inferred wrapper summary.
function throughWrapper(produce: () => unknown) { takeResult(produce); }
export function WrapperOpen() { throughWrapper(() => 1); return <div />; }
