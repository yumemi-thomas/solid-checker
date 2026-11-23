import * as S from "solid-js";
import { createMemo, createSignal, createTrackedEffect, flush, onCleanup } from "solid-js";

const [n] = createSignal(0);

// ADR 1180: a call that omits the argument runs the parameter's default, here in a tracked effect's callback.
const valued = (p: unknown = flush()) => p;
function drains() {
  flush();
}
const helped = (p: unknown = drains()) => p;
const spaced = (p: unknown = S.flush()) => p;

export function DefaultRuns() {
  createTrackedEffect(() => {
    n();
    valued();
  });
  return <p>x</p>;
}
export function DefaultHelperRuns() {
  createTrackedEffect(() => {
    n();
    helped();
  });
  return <p>x</p>;
}
export function DefaultNamespaceRuns() {
  createTrackedEffect(() => {
    n();
    spaced();
  });
  return <p>x</p>;
}

// The argument is passed: the default never runs. Clean.
export function ArgumentPassed() {
  createTrackedEffect(() => {
    n();
    valued(1);
  });
  return <p>x</p>;
}

// A spread argument: how many arguments arrive is unknown, so the default may or may not run. Never a violation.
const none: unknown[] = [];
export function SpreadArguments() {
  createTrackedEffect(() => {
    n();
    valued(...none);
  });
  return <p>x</p>;
}

// An earlier default that calls something: if it throws, the later default never runs. Never a violation.
function first(): unknown {
  return n();
}
const ordered = (a: unknown = first(), p: unknown = flush()) => [a, p];
export function EarlierDefaultCalls() {
  createTrackedEffect(() => {
    n();
    ordered();
  });
  return <p>x</p>;
}

// A local `flush` that is not Solid's: clean.
function shadowed() {
  const flush = () => 0;
  const local = (p: unknown = flush()) => p;
  return local;
}
export function ShadowedFlush() {
  const local = shadowed();
  createTrackedEffect(() => {
    n();
    local();
  });
  return <p>x</p>;
}

// A method's default, called through its object.
const holder = {
  run(p: unknown = flush()) {
    return p;
  },
};
export function MemberDefaultRuns() {
  createTrackedEffect(() => {
    n();
    holder.run();
  });
  return <p>x</p>;
}

// An argument that is the global `undefined` runs the default too (ADR 1188: a violation, as an omitted one).
export function UndefinedArgument() {
  createTrackedEffect(() => {
    n();
    valued(undefined);
  });
  return <p>x</p>;
}

// ADR 1188: a variable that holds `undefined` is a value, not the literal: it stays uncertifiable.
export function UndefinedThroughVariable() {
  const none = undefined;
  createTrackedEffect(() => {
    n();
    valued(none);
  });
  return <p>x</p>;
}

// A creation and a cleanup as the default.
const created = (p: unknown = createMemo(() => 1)) => p;
const cleaned = (p: unknown = onCleanup(() => {})) => p;
export function CreationDefaultRuns() {
  createTrackedEffect(() => {
    n();
    created();
  });
  return <p>x</p>;
}
export function CleanupDefaultRuns() {
  createTrackedEffect(() => {
    n();
    cleaned();
  });
  return <p>x</p>;
}
