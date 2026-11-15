import { Loading, createOptimistic, createSignal } from "solid-js";
import * as S from "solid-js";

// ADR 0987, slice 8a: every exit of SC5006 (`loading-on-outside-hold`), SC5007 (`loading-on-accessor`) and SC2009's producers is a finding or a typed, named
// silence; an exit that is neither is an uncertifiable row of the rule (`loading-on-outside-hold-unaccounted`, `loading-on-accessor-unaccounted`). Every
// site below ends in a named silence: no finding. With `SOLID_CHECKER_IR_ABLATE_SILENCE=all` each becomes an uncertifiable row of its rule.

// `on` names nothing (`on-names-nothing`: no identifier, no call, so no dependency to change) and is no function (`on-not-function`: the compiler's
// callability of the literal is non-callable).
export function NamesNothing() {
  return (
    <Loading fallback={0} on={1}>
      <p>x</p>
    </Loading>
  );
}

// `on` names a call, and nothing under the boundary reads an async source and every child is seen (`holds-no-async-source`); the call's value is a number
// (`on-not-function`).
export function HoldsNothing() {
  const [count] = createSignal(1);
  return (
    <Loading fallback={0} on={count()}>
      <p>x</p>
    </Loading>
  );
}

// The namespace member and an exact call enter the same boundary, by the dialect's identity of it.
export function Namespace() {
  return (
    <S.Loading fallback={0} on={2}>
      <p>x</p>
    </S.Loading>
  );
}

export function HandCall() {
  return Loading({ fallback: 0, on: 3 });
}

// A local function that happens to be named `Loading` is no boundary: no site, no finding.
export function Shadowed() {
  const Loading = (props: { on?: any; children?: unknown }) => <p>{props.children}</p>;
  return (
    <Loading on={4}>
      <p>x</p>
    </Loading>
  );
}

// A `createOptimistic(0)` setter in a handler: the source is synchronous (a plain initial value), so SC2009's code is not carried and nothing is reported.
export function SyncOptimistic() {
  const [value, setValue] = createOptimistic(0);
  return <button onClick={() => setValue(value() + 1)}>go</button>;
}
