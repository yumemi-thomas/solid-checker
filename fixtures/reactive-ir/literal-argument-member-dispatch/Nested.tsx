import { createSignal } from "solid-js";
import { readNested, escapingNested, replacingNested, defaultNested, callbackNested, computedNested, indexed, type NestedReader } from "./nested-source";
import * as readers from "./nested-source";

function inert() { return 1; }
declare function retain(value: unknown): void;

export function NestedLiteral() {
  const result = readNested({ source: { read: inert } });
  return <div>{result}</div>;
}

export function NestedBinding() {
  const value = { source: { read: inert } };
  const result = readNested(value);
  return <div>{result}</div>;
}

export function NestedAliases() {
  const inner = { read: inert };
  const holder = { source: inner };
  const alias = holder;
  const result = readNested((alias as NestedReader));
  return <div>{result}</div>;
}

export function NestedReactiveRead() {
  const [count] = createSignal(0);
  function reading() { return count(); }
  const result = readNested({ source: { read: reading } });
  return <div>{result}</div>;
}

export function NestedUnknown(props: { value: NestedReader }) {
  const result = readNested(props.value);
  return <div>{result}</div>;
}

export function NestedGetter() {
  const result = readNested({ get source() { return { read: inert }; } });
  return <div>{result}</div>;
}

export function NestedSpread() {
  const inner = { read: inert };
  const result = readNested({ source: { ...inner } });
  return <div>{result}</div>;
}

export function NestedMutable() {
  let value = { source: { read: inert } };
  const result = readNested(value);
  return <div>{result}</div>;
}

declare const replacement: NestedReader;
export function NestedReassignedLet() {
  let value = { source: { read: inert } };
  value = replacement;
  const result = readNested(value);
  return <div>{result}</div>;
}

export function NestedEscaped() {
  const value = { source: { read: inert } };
  retain(value);
  const result = readNested(value);
  return <div>{result}</div>;
}

export function NestedReplaced() {
  const value = { source: { read: inert } };
  value.source.read = () => 2;
  const result = readNested(value);
  return <div>{result}</div>;
}

export function NestedReceiver() {
  const result = readNested({ source: { read() { return this.read === inert ? 0 : 1; } } });
  return <div>{result}</div>;
}

export function NestedCalleeEscape() {
  const result = escapingNested({ source: { read: inert } });
  return <div>{result}</div>;
}

export function NestedCalleeWrite() {
  const result = replacingNested({ source: { read: inert } });
  return <div>{result}</div>;
}

export function NestedDefault() {
  const result = defaultNested({ source: { read: inert } });
  return <div>{result}</div>;
}

export function NestedDefaultUndefined() {
  const result = defaultNested(undefined);
  return <div>{result}</div>;
}

export function NestedDefaultUnknown(props: { value: NestedReader | undefined }) {
  const result = defaultNested(props.value);
  return <div>{result}</div>;
}

export function NestedConditional(props: { choose: boolean }) {
  const result = readNested(props.choose ? { source: { read: inert } } : { source: { read: inert } });
  return <div>{result}</div>;
}

function factory() { return inert; }
export function NestedFactory() {
  const result = readNested({ source: { read: factory() } });
  return <div>{result}</div>;
}

export function NestedWrongLevel() {
  const [count] = createSignal(0);
  function reading() { return count(); }
  const value = { read: inert, source: { read: reading } };
  const result = readNested(value);
  return <div>{result}</div>;
}

export function NestedNamespace() {
  const result = readers.readNested({ source: { read: inert } });
  return <div>{result}</div>;
}

export function NestedMethod() {
  const result = readNested({ source: { read() { return 1; } } });
  return <div>{result}</div>;
}

function invoke(cb: () => number) { return cb(); }
export function NestedMethodArguments() {
  const [count] = createSignal(0);
  const result = callbackNested({ source: { read: invoke } }, count);
  return <div>{result}</div>;
}

export function NestedComputedPrefix() {
  const result = computedNested({ outer: { source: indexed } }, "branch");
  return <div>{result}</div>;
}
