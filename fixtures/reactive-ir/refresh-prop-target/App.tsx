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

// ADR 1230: a component that hands the target's component its own props whole (`<InForward {...props} />`) passes what its own
// callers pass (ADR 1173's checked forwards): violations through one and two forwarders; uncertifiable where the forwarder sets
// `m1` itself or its callers pass two sources.
function InForward(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}
function Forwarder(props: { m1: SourceAccessor<number> }) {
  return <InForward {...props} />;
}
export function ForwardedParent() {
  const m1 = createMemo(() => 1);
  return <Forwarder m1={m1} />;
}

function InTwoLevels(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}
function Inner2(props: { m1: SourceAccessor<number> }) {
  return <InTwoLevels {...props} />;
}
function Outer2(props: { m1: SourceAccessor<number> }) {
  return <Inner2 {...props} />;
}
export function TwoLevelParent() {
  const m1 = createMemo(() => 1);
  return <Outer2 m1={m1} />;
}

function InOverride(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}
function Overrider(props: { m1: SourceAccessor<number> }) {
  const other = createMemo(() => 2);
  return <InOverride {...props} m1={other} />;
}
export function OverrideParent() {
  const m1 = createMemo(() => 1);
  return <Overrider m1={m1} />;
}

function InForwardTwo(props: { m1: SourceAccessor<number> }) {
  const m2 = createMemo(() => {
    refresh(props.m1);
    return 1;
  });
  return <p>{m2()}</p>;
}
function ForwarderTwo(props: { m1: SourceAccessor<number> }) {
  return <InForwardTwo {...props} />;
}
export function ForwardTwo() {
  const a = createMemo(() => 1);
  const b = createMemo(() => 2);
  return (
    <div>
      <ForwarderTwo m1={a} />
      <ForwarderTwo m1={b} />
    </div>
  );
}
