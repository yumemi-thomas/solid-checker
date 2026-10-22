import { createMemo, createSignal, createStore } from "solid-js";
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

// ADR 0338, round 4: an argument that is an expression rather than a symbol
// takes the same proof. Each of these reaches `access` as a getter at runtime.
export function UncertifiableLiteralArray() {
  const [get] = createSignal(1);
  const result = access([get][0]);
  return <span>{String(result)}</span>;
}

export function UncertifiableLiteralObject() {
  const [get] = createSignal(1);
  const result = access({ value: get }.value);
  return <span>{String(result)}</span>;
}

export function UncertifiableConditional() {
  const [get] = createSignal(1);
  const result = access(true ? get : get);
  return <span>{String(result)}</span>;
}

export function UncertifiableSpreadSlot() {
  const [get] = createSignal(1);
  const result = access([...createSignal(1)][0]);
  return <span>{String(result)}</span>;
}

export function UncertifiableIife() {
  const [get] = createSignal(1);
  const result = access((() => get)());
  return <span>{String(result)}</span>;
}

// A cast voids the primitive-wrapper exemption: the receiver is an object
// whose own `slice` returns the getter.
export function UncertifiableCastMethod() {
  const [get] = createSignal(1);
  const value = ({ slice: () => get } as unknown as string).slice(0);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableCastMethodDirect() {
  const [get] = createSignal(1);
  const result = access(({ slice: () => get } as unknown as string).slice(0));
  return <span>{String(result)}</span>;
}

// A call result's provenance is the callee's returns, not the callee.
function give() {
  return createSignal(1)[0];
}
export function UncertifiableProjectReturnHeld() {
  const value = access(give);
  const result = access(value);
  return <span>{String(result)}</span>;
}

export function UncertifiableProjectReturn() {
  const result = access(give());
  return <span>{String(result)}</span>;
}

// A store proxy is not callable, but a property of one may hold a getter.
export function UncertifiableStoreProperty() {
  const [get] = createSignal(1);
  const [store] = createStore({ value: get });
  const value = store.value;
  const result = access(value);
  return <span>{String(result)}</span>;
}

// Controls: a primitive-literal receiver, a String() receiver, a store proxy,
// and an all-literal conditional are proven non-getters.
export function GoodPrimitiveReceiver() {
  const result = access("abc".slice(0));
  return <span>{result}</span>;
}

export function GoodStringCallReceiver() {
  const value = String(3).slice(0);
  const result = access(value);
  return <span>{result}</span>;
}

export function GoodStoreProxy() {
  const [store] = createStore({ count: 1 });
  const result = access(store);
  return <span>{String(result.count)}</span>;
}

export function GoodLiteralConditional() {
  const result = access(Math.random() > 0.5 ? 1 : 2);
  return <span>{String(result)}</span>;
}
