import { createMemo } from "solid-js";
import * as H from "./helpers";
import { run } from "./helpers";
import { helpers as Api } from "./barrel";
import util from "./util";

const slow = createMemo(async () => {
  await Promise.resolve();
  return 1;
});

// ADR 1011: what a closure handed to a call says of the read it holds, when the callee is a MEMBER. No `Loading` stands anywhere here, so
// Chrome (rc.21) warns ASYNC_OUTSIDE_LOADING_BOUNDARY at the `slow()` read of every component of the first group, where the callee calls the
// closure while the component renders. The snapshot must hold a violation at those reads. A callee the analysis does not resolve exactly (a
// function that stores the closure, a method an assignment replaced, a function the component is handed) reads the closure as run where the
// analysis does not see, so a boundary may hold it: the read is uncertifiable, never a violation.

// First group: the callee is a project function the analysis resolves exactly, and it calls the closure in the render.
function Named() {
  return <p>{(() => { run(() => { slow(); }); return 1; })()}</p>;
}

function Namespace() {
  return <p>{(() => { H.run(() => { slow(); }); return 1; })()}</p>;
}

function Reexport() {
  return <p>{(() => { Api.run(() => { slow(); }); return 1; })()}</p>;
}

function DefaultMember() {
  return <p>{(() => { util.run(() => { slow(); }); return 1; })()}</p>;
}

function LiteralMethod() {
  const bus = { run(cb: () => unknown) { cb(); return 1; } };
  return <p>{(() => { bus.run(() => { slow(); }); return 1; })()}</p>;
}

// Second group: the callee is not resolved exactly, so the read is uncertifiable and no violation stands at it.
function Stored() {
  return <p>{(() => { H.keep(() => { slow(); }); return 1; })()}</p>;
}

function Replaced() {
  const bus = { run(cb: () => unknown) { cb(); return 1; } };
  bus.run = (cb: () => unknown) => { void cb; return 2; };
  return <p>{(() => { bus.run(() => { slow(); }); return 1; })()}</p>;
}

function Handed(props: { run: (cb: () => unknown) => number }) {
  return <p>{(() => { props.run(() => { slow(); }); return 1; })()}</p>;
}

export { Named, Namespace, Reexport, DefaultMember, LiteralMethod, Stored, Replaced, Handed };
