import { Loading, Loading as Hold, createComponent, createMemo, dynamicComponent } from "solid-js";
import * as S from "solid-js";
import { createLoadingBoundary } from "solid-js/internal";
import { render } from "@solidjs/web";

const slow = createMemo(async () => {
  await Promise.resolve();
  return 1;
});
const other = createMemo(async () => {
  await Promise.resolve();
  return 2;
});

// ADR 0978: what covers a read when the boundary is entered other than as a tag. Each component reads `slow()` where its boundary
// holds it and `other()` beside it. Chrome (rc.21) is quiet at the `slow()` read of the first group and warns
// ASYNC_OUTSIDE_LOADING_BOUNDARY at every `other()` read, so the snapshot must hold a violation at `other()` and none at the
// `slow()` read in a component of the first group. Where the boundary is a form the analysis does not follow, the `slow()` read
// is uncertifiable, never a violation: a layer a call hands to unresolved code may be run inside a boundary.

// First group: the `children` getter (or the first argument of `createLoadingBoundary`) is what the boundary holds. `JsxTag` is the control: the
// tag holds `slow()` and no row stands at it.
function JsxTag() {
  return (
    <div>
      <Loading>
        <p>{slow()}</p>
      </Loading>
      <p>{other()}</p>
    </div>
  );
}

function HandCall() {
  return <div>{Loading({ get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

function AliasCall() {
  return <div>{Hold({ get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

function NamespaceCall() {
  return <div>{S.Loading({ get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

const Local = Loading;
function LocalAliasCall() {
  return <div>{Local({ get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

function ComponentCall() {
  return <div>{createComponent(Loading, { get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

function IifeCall() {
  return <div>{(() => Loading({ get children() { return <p>{slow()}</p>; } }))()}<p>{other()}</p></div>;
}

function BoundaryPrimitive() {
  const view = createLoadingBoundary(() => <p>{slow()}</p>, () => <p>f</p>);
  return <div>{view()}<p>{other()}</p></div>;
}

// The fallback renders outside the hold: a read in it is read outside the boundary, whether the boundary is a tag, a call or the
// primitive. Violations at the `other()` reads in the three (Chrome warns in each).
function FallbackTag() {
  return (
    <div>
      <Loading fallback={<p>{other()}</p>}>
        <p>{slow()}</p>
      </Loading>
    </div>
  );
}

function FallbackGetter() {
  return <div>{Loading({ get fallback() { return <p>{other()}</p>; }, get children() { return <p>{slow()}</p>; } })}</div>;
}

function FallbackPrimitive() {
  const view = createLoadingBoundary(() => <p>{slow()}</p>, () => <p>{other()}</p>);
  return <div>{view()}</div>;
}

// A boundary's `on` is read inside the hold (Chrome is quiet at it); the call form's getter is a member the analysis does not
// place, so the read is uncertifiable, as is every member that is not the `children` getter.
function OnGetter() {
  return <div>{Loading({ get on() { return other(); }, get children() { return <p>{slow()}</p>; } })}</div>;
}

function FunctionValuedChildren() {
  return <div>{Loading({ children: (() => <p>{slow()}</p>) as any })}</div>;
}

// Second group: the boundary is a form the analysis does not follow, so what its layers run inside is unknown. Uncertifiable at
// the `slow()` read, a violation at `other()`.
function DotCall() {
  return <div>{Loading.call(null, { get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

function ArrayCall() {
  return <div>{[Loading][0]({ get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

function holdWith(props: { children?: any }) {
  return Loading(props);
}
function WrapperCall() {
  return <div>{holdWith({ get children() { return <p>{slow()}</p>; } })}<p>{other()}</p></div>;
}

const Dyn = dynamicComponent(() => Loading);
function DynamicTag() {
  return (
    <div>
      <Dyn>
        <p>{slow()}</p>
      </Dyn>
      <p>{other()}</p>
    </div>
  );
}

// Control: no boundary at all. A violation at both reads.
function NoBoundary() {
  return <div><p>{slow()}</p><p>{other()}</p></div>;
}

render(() => <JsxTag />, document.body);
render(() => <HandCall />, document.body);
render(() => <AliasCall />, document.body);
render(() => <NamespaceCall />, document.body);
render(() => <LocalAliasCall />, document.body);
render(() => <ComponentCall />, document.body);
render(() => <IifeCall />, document.body);
render(() => <BoundaryPrimitive />, document.body);
render(() => <FallbackTag />, document.body);
render(() => <FallbackGetter />, document.body);
render(() => <FallbackPrimitive />, document.body);
render(() => <OnGetter />, document.body);
render(() => <FunctionValuedChildren />, document.body);
render(() => <DotCall />, document.body);
render(() => <ArrayCall />, document.body);
render(() => <WrapperCall />, document.body);
render(() => <DynamicTag />, document.body);
render(() => <NoBoundary />, document.body);
