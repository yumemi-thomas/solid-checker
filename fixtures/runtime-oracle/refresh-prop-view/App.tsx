import { createMemo, merge, omit, refresh } from "solid-js";
import type { SourceAccessor } from "solid-js";

// ADR 1214 (slice A): `refresh(view.m1)` on a `const` `merge(literal, props)` or `omit(props, "k")` view, whose Get of `m1` is the props
// parameter's own (ADR 1100's view semantics), names `props.m1`; every caller passes one exact memo there (ADR 1174), so the write is that
// memo's, placed by the call's role. Written in a component body, it is owned: rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE.

type Props = { m1: SourceAccessor<number>; extra: number };

// Violation: a merge view with the props last.
function MergeView(props: Props) {
  const merged = merge({ other: 1 }, props);
  refresh(merged.m1);
  return <p>a</p>;
}
export function MergeParent() {
  const m1 = createMemo(() => 1);
  return <MergeView m1={m1} extra={1} />;
}

// Violation: an omit view hiding another key.
function OmitView(props: Props) {
  const rest = omit(props, "extra");
  refresh(rest.m1);
  return <p>b</p>;
}
export function OmitParent() {
  const m1 = createMemo(() => 1);
  return <OmitView m1={m1} extra={1} />;
}

// Clean: the same view's refresh in an event handler.
function HandlerView(props: Props) {
  const merged = merge({ other: 1 }, props);
  return <button onClick={() => refresh(merged.m1)}>c</button>;
}
export function HandlerParent() {
  const m1 = createMemo(() => 1);
  return <HandlerView m1={m1} extra={1} />;
}

// Kept uncertifiable: the props are not last, so a later source may answer `m1`.
function PropsFirst(props: Props) {
  const merged = merge(props, { other: 1 });
  refresh(merged.m1);
  return <p>d</p>;
}
export function PropsFirstParent() {
  const m1 = createMemo(() => 1);
  return <PropsFirst m1={m1} extra={1} />;
}

// Kept uncertifiable (the shadowed variant): the view is a `let`, written again.
function Rebound(props: Props) {
  let view = merge({ other: 1 }, props);
  view = merge({ other: 2 }, props);
  refresh(view.m1);
  return <p>e</p>;
}
export function ReboundParent() {
  const m1 = createMemo(() => 1);
  return <Rebound m1={m1} extra={1} />;
}

// Kept uncertifiable (the member variant): a view of a view.
function ViewOfView(props: Props) {
  const inner = merge({ other: 1 }, props);
  const outer = merge({ more: 1 }, inner);
  refresh(outer.m1);
  return <p>f</p>;
}
export function ViewOfViewParent() {
  const m1 = createMemo(() => 1);
  return <ViewOfView m1={m1} extra={1} />;
}
