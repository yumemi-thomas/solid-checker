import { createEffect, createMemo, refresh } from "solid-js";
import type { SourceAccessor } from "solid-js";

// ADR 1174: `refresh(props.m1)` in a memo's compute, where every caller passes `m1` one exact memo: the write is that
// memo's, placed by the call's own role (rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE).

function InMemo(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}

// Violation: the one caller passes a memo.
export function MemoParent() {
  const m1 = createMemo(() => 1);
  return <InMemo m1={m1} />;
}

function InEffectCompute(props: { m1: SourceAccessor<number> }) {
  createEffect(
    () => {
      refresh(props.m1);
      return 1;
    },
    () => {},
  );
  return <p />;
}

// Violation: an effect's compute is owned too.
export function EffectParent() {
  const m1 = createMemo(() => 1);
  return <InEffectCompute m1={m1} />;
}

function Destructured({ m1 }: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(m1);
    return 1;
  });
  return <p>{m2()}</p>;
}

// Violation: a destructured props slot is the passed memo too.
export function DestructuredParent() {
  const m1 = createMemo(() => 1);
  return <Destructured m1={m1} />;
}

function InHandler(props: { m1: SourceAccessor<number> }) {
  return <button onClick={() => void refresh(props.m1)}>x</button>;
}

// Clean: a handler runs outside every owned scope.
export function HandlerParent() {
  const m1 = createMemo(() => 1);
  return <InHandler m1={m1} />;
}

function TwoCallers(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}

// Uncertifiable: two callers pass two sources.
export function FirstCaller() {
  const a = createMemo(() => 1);
  return <TwoCallers m1={a} />;
}

export function SecondCaller() {
  const b = createMemo(() => 2);
  return <TwoCallers m1={b} />;
}

function Spread(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}

// Uncertifiable: a spread caller is not enumerated.
export function SpreadParent() {
  const m1 = createMemo(() => 1);
  const rest = { m1 };
  return <Spread {...rest} />;
}

function Written(props: { m1: SourceAccessor<number> }) {
  props.m1 = createMemo(() => 3);
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}

// Uncertifiable: the component writes its own `props.m1`, so the target is not proven the passed memo.
export function WrittenParent() {
  const m1 = createMemo(() => 1);
  return <Written m1={m1} />;
}
