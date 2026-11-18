import { createMemo, createSignal, merge, omit } from "solid-js";
import type { Accessor } from "solid-js";

// ADR 1100: a member call on a `const` merge or omit view whose rc.21 Get is
// the props parameter's own Get is the prop's call: here a tracked read of
// the accessor every caller passes.

type P = { r1: Accessor<number>; extra?: number };

// Proved: the parameter last in `merge`, the literal before it without `r1`.
function MergeRead(props: P) {
  const merged = merge({ extra: 1 }, props);
  const m1 = createMemo(() => {
    merged.r1();
    return 1;
  });
  return <p>{m1()}</p>;
}
// Proved: `omit` hides string-literal keys other than `r1`.
function OmitRead(props: P) {
  const rest = omit(props, "extra");
  const m1 = createMemo(() => {
    rest.r1();
    return 1;
  });
  return <p>{m1()}</p>;
}
// Misuse: the same view read untracked in the component body.
function MergeBody(props: P) {
  const merged = merge({ extra: 1 }, props);
  merged.r1();
  return <p />;
}
function OmitBody(props: P) {
  const rest = omit(props, "extra");
  rest.r1();
  return <p />;
}
// Refused: a source after the parameter may answer `r1` first.
function ParameterFirst(props: P) {
  const merged = merge(props, { extra: 1 });
  const m1 = createMemo(() => {
    merged.r1();
    return 1;
  });
  return <p>{m1()}</p>;
}
// Refused: an earlier source that is no literal may hold `r1`.
const defaults = JSON.parse("{}") as { extra?: number };
function UnknownSource(props: P) {
  const merged = merge(defaults, props);
  const m1 = createMemo(() => {
    merged.r1();
    return 1;
  });
  return <p>{m1()}</p>;
}
// Refused: a predicate hides keys nothing names.
function OmitPredicate(props: P) {
  const rest = omit(props, (key) => key === "extra");
  const m1 = createMemo(() => {
    rest.r1!();
    return 1;
  });
  return <p>{m1()}</p>;
}
// Refused: a computed member, and a view of a view.
function Computed(props: P) {
  const merged = merge({ extra: 1 }, props);
  const m1 = createMemo(() => {
    merged["r1"]();
    return 1;
  });
  return <p>{m1()}</p>;
}
function Nested(props: P) {
  const merged = merge({ extra: 1 }, omit(props, "extra"));
  const m1 = createMemo(() => {
    merged.r1();
    return 1;
  });
  return <p>{m1()}</p>;
}
// Refused: a local `merge` shadows the import.
function Shadowed(props: P) {
  const merge = (a: object, b: P) => ({ ...a, ...b });
  const merged = merge({ extra: 1 }, props);
  const m1 = createMemo(() => {
    merged.r1();
    return 1;
  });
  return <p>{m1()}</p>;
}

export function App() {
  const [r1] = createSignal(3);
  return (
    <p>
      <MergeRead r1={r1} />
      <OmitRead r1={r1} />
      <MergeBody r1={r1} />
      <OmitBody r1={r1} />
      <ParameterFirst r1={r1} />
      <UnknownSource r1={r1} />
      <OmitPredicate r1={r1} />
      <Computed r1={r1} />
      <Nested r1={r1} />
      <Shadowed r1={r1} />
    </p>
  );
}
