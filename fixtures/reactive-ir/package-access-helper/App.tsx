import { createMemo, createSignal, createStore } from "solid-js";
import { access, accessWith } from "@solid-primitives/utils";
import { access as callsWhenParameterless } from "./local-access";
import { access as callsEveryFunction } from "./local-calls";
import { callsLater, redefines } from "./hoisted";

const config = { label: "static" };

// Positive: a signal getter is `typeof "function"` with `length` 0, so the
// package's `access` calls it on every path, here in the component body.
export function BadSignal() {
  const [value] = createSignal(1);
  const initial = access(value);
  return <span>{initial}</span>;
}

// Positive: a memo getter is built the same way.
export function BadMemo() {
  const [value] = createSignal(1);
  const doubled = createMemo(() => value() * 2);
  const initial = access(doubled);
  return <span>{initial}</span>;
}

// Positive: `accessWith` calls any function argument, whatever its length.
export function BadAccessWith() {
  const [value] = createSignal(1);
  const initial = accessWith(value);
  return <span>{initial}</span>;
}

// Positive: a project function named `access` follows its own body, which
// calls every function.
export function BadLocalCallsEveryFunction() {
  const [value] = createSignal(1);
  const initial = callsEveryFunction(value);
  return <span>{initial}</span>;
}

// Negative: the call is a tracked JSX expression.
export function GoodTracked() {
  const [value] = createSignal(1);
  return <span>{access(value)}</span>;
}

// Negative: the call is inside a memo's compute, which tracks.
export function GoodMemoCompute() {
  const [value] = createSignal(1);
  const doubled = createMemo(() => access(value) * 2);
  return <span>{doubled()}</span>;
}

// Negative: plain values are returned as they are.
export function GoodPlain() {
  const plain = 5;
  const label = access(config.label);
  return <span>{access(plain)}{label}</span>;
}

// Negative: a store is an object, not a function, so `access` returns it.
export function GoodStore() {
  const [state] = createStore({ count: 1 });
  const snapshot = access(state);
  return <span>{String(snapshot)}</span>;
}

// Negative: a function with a parameter has `length` 1, so `access` returns
// it without calling it.
export function GoodWithParameters() {
  const addOne = (n: number) => n + 1;
  const result = access(addOne);
  return <span>{String(result)}</span>;
}

// Negative: a project function named `access` follows its own body, which
// calls only a function with a parameter; a getter has none.
export function GoodLocalCallsWithParameters() {
  const [value] = createSignal(1);
  const initial = callsWhenParameterless(value);
  return <span>{initial}</span>;
}

// Uncertifiable: an alias is not the binding the primitive created, so the
// argument proves nothing about its arity.
export function UncertifiableAlias() {
  const [value] = createSignal(1);
  const alias = value;
  const initial = access(alias);
  return <span>{initial}</span>;
}

// Negative: a member call on a primitive's result is that value's own method,
// never the primitive. `createMemo(...).toString()` is a string, and
// `createSignal(1).slice(1)` and `.reverse()` are arrays whose first item is
// the setter; none is a getter, so nothing is read and nothing is reported.
export function UncertifiableMemoToString() {
  const text = createMemo(() => 2).toString();
  const initial = access(text);
  return <span>{String(initial)}</span>;
}

export function UncertifiableSlice() {
  const [value] = createSignal(1).slice(1);
  const initial = access(value);
  return <span>{String(initial)}</span>;
}

export function UncertifiableReverse() {
  const [value] = createSignal(1).reverse();
  const initial = access(value);
  return <span>{String(initial)}</span>;
}

// Uncertifiable: `length` is configurable, so a getter handed to
// `Object.defineProperty` may no longer have length 0.
export function UncertifiableRedefinedLength() {
  const [value] = createSignal(1);
  Object.defineProperty(value, "length", { value: 1 });
  const initial = access(value);
  return <span>{initial}</span>;
}

// Uncertifiable: the getter is handed to another function before the call.
export function UncertifiablePassedElsewhere() {
  const [value] = createSignal(1);
  const keep: Array<unknown> = [];
  keep.push(value);
  const initial = access(value);
  return <span>{initial}</span>;
}

// Positive: calling the getter, and handing it to the same helper twice, do
// not refuse the fact.
export function BadCalledAndRepeated() {
  const [value] = createSignal(1);
  const first = access(value);
  const second = access(value);
  return <span>{first + second + value()}</span>;
}

// Uncertifiable: the callee redefines `length` in a hoisted function
// declaration that runs before its `return`; a later site is not
// unreachable for being written after that `return`.
export function UncertifiableHoistedRedefinition() {
  const [value] = createSignal(1);
  const result = redefines(value);
  return <span>{String(result)}</span>;
}

// Known miss, silent: the callee reads the getter in a hoisted function it
// calls; nested function declarations are not followed (docs/open-work.md).
export function MissedHoistedCallee() {
  const [value] = createSignal(1);
  const result = callsLater(value);
  return <span>{String(result)}</span>;
}
