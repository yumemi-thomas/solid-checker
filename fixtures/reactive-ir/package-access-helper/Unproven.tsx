import { createMemo, createSignal } from "solid-js";
import { access } from "@solid-primitives/utils";

// ADR 0338, inverted: `access` calls its argument unless the argument is
// proven not to be a function. A getter that reaches it by a route the
// analysis does not follow (a spread, a rest pattern, a held alias, a helper's
// return, `Reflect.apply`) is called all the same (rc.13 emits
// STRICT_READ_UNTRACKED for each of the uncertifiable cases below), so each
// keeps an SC9012 obligation. Only a value proven not to be a getter settles
// the call: the `Good*` components.

function keep<T>(value: T) {
  return value;
}
function consume(get: () => number) {
  return access(get);
}
function copy<T>(pair: T[]) {
  return pair.slice(0);
}

export function UncertifiableSpread() {
  const [value] = [...createSignal(1)];
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableArrayFrom() {
  const [value] = Array.from(createSignal(1));
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableObjectTuple() {
  const box = { pair: createSignal(1) };
  const [value] = box.pair;
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableObjectSlice() {
  const box = { pair: createSignal(1) };
  const [value] = box.pair.slice(0);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableRest() {
  const [...pair] = createSignal(1);
  const [value] = pair;
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableHeldAlias() {
  const pair = createSignal(1);
  const alias = pair;
  const [value] = alias.slice(0);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableBoundGetter() {
  const [get] = createSignal(1);
  const value = get.bind(null);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableReflectApply() {
  const memo = createMemo(() => 1);
  const value = Reflect.apply(Function.prototype.bind, memo, [null]);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableHelperReturn() {
  const [get] = createSignal(1);
  const value = keep(get);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableHelperHop() {
  const [get] = createSignal(1);
  const value = consume(get);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableHelperSlice() {
  const pair = createSignal(1);
  const [value] = copy(pair);
  const result = access(value);
  return <span>{String(result)}</span>;
}

// Negative: a project function is never a getter (only a primitive produces
// one), so `access` calling it reads nothing reactive.
function plain() {
  return 1;
}
export function GoodProjectFunction() {
  const result = access(plain);
  return <span>{String(result)}</span>;
}

// Negative: a literal and a primitive-typed value are not callable.
export function GoodLiteral() {
  const result = access(3);
  return <span>{String(result)}</span>;
}

const label: string = String(Math.random());
export function GoodPrimitiveTyped() {
  const result = access(label);
  return <span>{result}</span>;
}
