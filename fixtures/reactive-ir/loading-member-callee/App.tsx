import { createMemo } from "solid-js";
import { render } from "@solidjs/web";
import * as H from "./helpers";
import { both, branch, chain0, exprArrow, forwardBoundary, ignore, inBoundary, mixed, returned, run, viaArguments, visitBoundary } from "./helpers";
import * as J from "./jsxhelpers";
import { jsxBoundary } from "./jsxhelpers";
import { createLoadingBoundary } from "solid-js/internal";
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
// render, or hands it down a chain of project functions that do (`Forward`, `PassOn`). `Named` and `NamedReexport` are the identifier controls (they held before); `Namespace` is `H.run(cb)` of a namespace import
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

// Second group: the callee is not resolved exactly, or its own body puts a boundary around its calls of the parameter, or does not call it
// where it runs (a closure it stores, runs later, calls only on a flag), so the read is uncertifiable and no violation stands at it.
// Chrome is quiet at the reads of `NsInBoundary`, `NsInBoundaryBlock`, `IdInBoundary` and `IdInBoundaryBlock`: the callee's boundary catches them.
function NsInBoundary() {
  return <p>{(() => { H.exprArrow(() => { slow(); }); return 1; })()}</p>;
}

function NsInBoundaryBlock() {
  return <p>{(() => { H.inBoundary(() => { slow(); }); return 1; })()}</p>;
}

function IdInBoundary() {
  return <p>{(() => { exprArrow(() => { slow(); }); return 1; })()}</p>;
}

function IdInBoundaryBlock() {
  return <p>{(() => { inBoundary(() => { slow(); }); return 1; })()}</p>;
}

function Later() {
  return <p>{(() => { H.later(() => { slow(); }); return 1; })()}</p>;
}

function Maybe() {
  return <p>{(() => { H.maybe(() => { slow(); }, true); return 1; })()}</p>;
}

function PassOn() {
  return <p>{(() => { H.passOn(() => { slow(); }); return 1; })()}</p>;
}

function Forward() {
  return <p>{(() => { H.forward(() => { slow(); }); return 1; })()}</p>;
}

function ForwardBoundary() {
  return <p>{(() => { H.forwardBoundary(() => { slow(); }); return 1; })()}</p>;
}

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

// ADR 1011 (review of the first cut): the callee's boundary decides only where it holds EVERY use of the parameter that can run the closure.
// Chrome warns at the read of each of these components (a bare call runs in the render). Violations, as on main (the second cut lost them): `BothNs`,
// `BothId`, `ViaArgumentsId`, `WrapLet`, `ViaWrap`. `BranchId` and `ReturnedId` are uncertifiable on main too (`call-attributed-read`: a call on a flag, or in a
// closure the callee returns, is not proved to run in the render, as `Maybe`): pinned, not claimed, and the walk must not turn them into a violation.
function BothNs() {
  return <p>{(() => { H.both(() => { slow(); }); return 1; })()}</p>;
}

function BothId() {
  return <p>{(() => { both(() => { slow(); }); return 1; })()}</p>;
}

function BranchId() {
  return <p>{(() => { branch(() => { slow(); }, false); return 1; })()}</p>;
}

function ReturnedId() {
  return <p>{(() => { returned(() => { slow(); })(); return 1; })()}</p>;
}

function ViaArgumentsId() {
  return <p>{(() => { viaArguments(() => { slow(); }); return 1; })()}</p>;
}

// An identifier callee is the function its name runs only where nothing replaces it. `wrapLet` is reassigned to a function that runs the
// closure bare, `wrapped` is what `ignore` returns (the same): Chrome warns at both reads, so the violation stays and the boundary written in
// the first function is not the callee's.
let wrapLet = (cb: () => unknown) => {
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
};
wrapLet = (cb: () => unknown) => {
  cb();
  return 2;
};
function WrapLet() {
  return <p>{(() => { wrapLet(() => { slow(); }); return 1; })()}</p>;
}

const wrapped = ignore((cb: () => unknown) => {
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
});
function ViaWrap() {
  return <p>{(() => { wrapped(() => { slow(); }); return 1; })()}</p>;
}

// The review of the third cut: shapes the walk must pin. No `Loading` stands in the caller; Chrome is quiet at the reads of `JsxNs`, `JsxId`,
// `ForwardBoundaryId`, `Visitor` and `ArgLenNs` (a boundary holds the call) and warns at `Alias` and `Chain17` (a bare call runs in the render, as in
// `Namespace`). `Mixed` has one held use and one the walk cannot place.
function JsxNs() {
  return <p>{(() => { J.jsxBoundary(() => { slow(); return "x"; }); return 1; })()}</p>;
}

function JsxId() {
  return <p>{(() => { jsxBoundary(() => { slow(); return "x"; }); return 1; })()}</p>;
}

function ForwardBoundaryId() {
  return <p>{(() => { forwardBoundary(() => { slow(); }); return 1; })()}</p>;
}

const K = H;
function Alias() {
  return <p>{(() => { K.run(() => { slow(); }); return 1; })()}</p>;
}

function Visitor() {
  return <p>{(() => { visitBoundary({ v: 1 }, () => { slow(); }); return 1; })()}</p>;
}

function MixedNs() {
  return <p>{(() => { H.mixed(() => { slow(); }); return 1; })()}</p>;
}

function MixedId() {
  return <p>{(() => { mixed(() => { slow(); }); return 1; })()}</p>;
}

function ArgLenNs() {
  return <p>{(() => { H.argLen(() => { slow(); }); return 1; })()}</p>;
}

function Chain17() {
  return <p>{(() => { chain0(() => { slow(); }); return 1; })()}</p>;
}

render(() => <Named />, document.body);
render(() => <NamedReexport />, document.body);
render(() => <Namespace />, document.body);
render(() => <Reexport />, document.body);
render(() => <DefaultMember />, document.body);
render(() => <LiteralMethod />, document.body);
render(() => <NsInBoundary />, document.body);
render(() => <NsInBoundaryBlock />, document.body);
render(() => <IdInBoundary />, document.body);
render(() => <IdInBoundaryBlock />, document.body);
render(() => <Later />, document.body);
render(() => <Maybe />, document.body);
render(() => <PassOn />, document.body);
render(() => <Forward />, document.body);
render(() => <ForwardBoundary />, document.body);
render(() => <Stored />, document.body);
render(() => <Replaced />, document.body);
render(() => <Shadowed />, document.body);
render(() => <Handed run={(cb) => { cb(); return 1; }} />, document.body);
render(() => <BothNs />, document.body);
render(() => <BothId />, document.body);
render(() => <BranchId />, document.body);
render(() => <ReturnedId />, document.body);
render(() => <ViaArgumentsId />, document.body);
render(() => <WrapLet />, document.body);
render(() => <ViaWrap />, document.body);
render(() => <JsxNs />, document.body);
render(() => <JsxId />, document.body);
render(() => <ForwardBoundaryId />, document.body);
render(() => <Alias />, document.body);
render(() => <Visitor />, document.body);
render(() => <MixedNs />, document.body);
render(() => <MixedId />, document.body);
render(() => <ArgLenNs />, document.body);
render(() => <Chain17 />, document.body);
