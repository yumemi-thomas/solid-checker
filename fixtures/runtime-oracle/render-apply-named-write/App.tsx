import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: each export is the matching reactive-ir fixture `render-apply-named-write(-handed-twice, -deferred)` and
// `render-apply-child-write(-wrapped)`.
// rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE in `Named`, `HandedTwice`, `ChildWrite` and `ChildWrapped`, not in
// `Deferred`. `CallingParent` is `child-mount-first-run`'s (ADR 0507's twin, now a violation): it throws there too, as in
// `PropParent` and `ConditionalParent` (its condition false at run time; the fixture's is an ambient `declare const`).

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

function WriteChildCalling() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(2);
    },
  );
  return <p>{count()}</p>;
}

export function CallingParent() {
  const [flag] = createSignal(true);
  void flag;
  return (
    <div>
      <WriteChildCalling />
    </div>
  );
}

function WriteChildProp(props: { n: number }) {
  const [count, setCount] = createSignal(props.n);
  createRenderEffect(
    () => 1,
    () => {
      setCount(4);
    },
  );
  return <p>{count()}</p>;
}

export function PropParent() {
  return (
    <div>
      <WriteChildProp n={1} />
    </div>
  );
}

function WriteChildConditional() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(5);
    },
  );
  return <p>{count()}</p>;
}

const wide = Math.random() > 2;

export function ConditionalParent() {
  if (wide) {
    return <span />;
  }
  return (
    <div>
      <WriteChildConditional />
    </div>
  );
}
