import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: each export is the matching reactive-ir fixture `render-apply-named-write(-handed-twice, -deferred)` and
// `render-apply-child-write(-wrapped)`.
// rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE in `Named`, `HandedTwice`, `ChildWrite` and `ChildWrapped`, not in
// `Deferred`.

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

function ChildChildWrite(props: { set: (value: number) => number; extra: number }) {
  createRenderEffect(
    () => 1,
    (v) => {
      props.set(v);
    },
  );
  return <p />;
}

export function ChildWrite() {
  const [, setW] = createSignal(0);
  return (
    <div>
      <ChildChildWrite set={setW} extra={1} />
    </div>
  );
}

function ChildChildWrapped(props: { set: (value: number) => number; extra: number }) {
  createRenderEffect(
    () => 1,
    (v) => {
      props.set(v);
    },
  );
  return <p />;
}

function WrapChildWrapped(props: { children?: unknown }) {
  return <div>{props.children as never}</div>;
}

export function ChildWrapped() {
  const [, setW] = createSignal(0);
  return (
    <WrapChildWrapped>
      <ChildChildWrapped set={setW} extra={1} />
    </WrapChildWrapped>
  );
}
