import { createMemo } from "solid-js";
import { render } from "@solidjs/web";
import * as H from "./helpers";
import { run } from "./helpers";
import { go } from "./reexport";
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

// First group: the callee is a project function the analysis resolves exactly and the call graph places the closure in: it calls it in the
// render. `Named` and `NamedReexport` are the identifier controls (they held before); `Namespace` is `H.run(cb)` of a namespace import
// (a violation until ADR 1011's fix was an SC5003 `mount-unresolved`). The member shapes below it (a re-exported namespace, a default
// import's method, a literal's method) are resolved by no call edge to the closure, so they stay uncertifiable: pinned, not claimed.
function Named() {
  return <p>{(() => { run(() => { slow(); }); return 1; })()}</p>;
}

function NamedReexport() {
  return <p>{(() => { go(() => { slow(); }); return 1; })()}</p>;
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

function Shadowed() {
  const H = { run(cb: () => unknown) { void cb; return 3; } };
  return <p>{(() => { H.run(() => { slow(); }); return 1; })()}</p>;
}

function Handed(props: { run: (cb: () => unknown) => number }) {
  return <p>{(() => { props.run(() => { slow(); }); return 1; })()}</p>;
}

render(() => <Named />, document.body);
render(() => <NamedReexport />, document.body);
render(() => <Namespace />, document.body);
render(() => <Reexport />, document.body);
render(() => <DefaultMember />, document.body);
render(() => <LiteralMethod />, document.body);
render(() => <Stored />, document.body);
render(() => <Replaced />, document.body);
render(() => <Shadowed />, document.body);
render(() => <Handed run={(cb) => { cb(); return 1; }} />, document.body);
