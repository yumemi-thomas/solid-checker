import { Loading, Loading as Hold, createComponent, createMemo, createSignal, dynamicComponent } from "solid-js";
import * as S from "solid-js";
import { createLoadingBoundary } from "solid-js/internal";
import { Dynamic } from "@solidjs/web";

const [t, setT] = createSignal(1);
const a = createMemo(async () => {
  t();
  await Promise.resolve();
  return 1;
});

// ADR 0978: `Loading` is a component function, and a program enters it other than as a tag. SC5006 (an `on` that holds a read the
// boundary re-arms) and SC5007 (an accessor handed as `on`) are asked of every entry, with a row where `on` may be passed, and
// never a certified clean. Chrome (rc.21) warns LOADING_ON_OUTSIDE_HOLD after the click in each program of the first group, and
// LOADING_ON_ACCESSOR on the first run of each of the second.

// First group: SC5006 is uncertifiable at the `on` property (or at what hands it).
export function JsxTag() {
  return (
    <div>
      <Loading fallback={0} on={t()}>
        <p>{a()}</p>
      </Loading>
      <p>{a()}</p>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}

export function HandCall() {
  return <div>{Loading({ get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

export function AliasCall() {
  return <div>{Hold({ get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

export function NamespaceCall() {
  return <div>{S.Loading({ get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

const Local = Loading;
export function LocalAliasCall() {
  return <div>{Local({ get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

export function ComponentCall() {
  return <div>{createComponent(Loading, { get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

export function IifeCall() {
  return <div>{(() => Loading({ get on() { return t(); }, get children() { return <p>{a()}</p>; } }))()}<p>{a()}</p></div>;
}

// The props are not a literal: `on` may be in them.
function holdWith(props: { on?: any; children?: any }) {
  return Loading(props);
}
export function WrapperCall() {
  return <div>{holdWith({ get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

// A value reference: the function reaches code the analysis does not follow (SC5006 and SC5007 at the reference).
export function DotCall() {
  return <div>{Loading.call(null, { get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

export function ArrayCall() {
  return <div>{[Loading][0]({ get on() { return t(); }, get children() { return <p>{a()}</p>; } })}<p>{a()}</p></div>;
}

const Dyn = dynamicComponent(() => Loading);
export function DynamicTag() {
  return (
    <div>
      <Dyn fallback={0} on={t()}>
        <p>{a()}</p>
      </Dyn>
      <p>{a()}</p>
    </div>
  );
}

export function DynamicElement() {
  return (
    <div>
      <Dynamic component={Loading} fallback={0} on={t()}>
        <p>{a()}</p>
      </Dynamic>
      <p>{a()}</p>
    </div>
  );
}

// The tag a parameter holds is not resolved: it may be the boundary.
function Gate(props: any) {
  return <props.as on={props.on}>{props.children}</props.as>;
}
export function ParameterTag() {
  return (
    <div>
      <Gate as={Loading} on={t()}>
        <p>{a()}</p>
      </Gate>
      <p>{a()}</p>
    </div>
  );
}

// Second group: SC5007. A function proven (a typed accessor) is a violation where the call is exact; a value that may be one, or
// handed to a form the analysis does not follow, is uncertifiable.
export function AccessorHand() {
  return <div>{Loading({ on: t, children: <p>x</p> })}</div>;
}

export function AccessorComponent() {
  return <div>{createComponent(Loading, { on: t, children: <p>x</p> })}</div>;
}

export function AccessorDotCall() {
  return <div>{Loading.call(null, { on: t, children: <p>x</p> })}</div>;
}

export function AccessorArray() {
  return <div>{[Loading][0]({ on: t, children: <p>x</p> })}</div>;
}

// Stays clean: a value Type Facts prove is no function states nothing about SC5007, and a literal closed against `on` has none.
export function ValueComponent() {
  return <div>{createComponent(Loading, { on: t(), children: <p>x</p> })}</div>;
}

export function NoOnComponent() {
  return <div>{createComponent(Loading, { get children() { return <p>{a()}</p>; } })}</div>;
}

export function NoOnHand() {
  return <div>{Loading({ fallback: 0, get children() { return <p>{a()}</p>; } })}</div>;
}

export function ConstantOnHand() {
  return <div>{Loading({ on: 1, get children() { return <p>{a()}</p>; } })}</div>;
}

// A literal with accessors is closed only while no spread and no computed key can add an `on`: these two keep their row.
declare const extra: { on?: () => number };
export function SpreadLiteral() {
  return <div>{createComponent(Loading, { get children() { return <p>{a()}</p>; }, ...extra })}</div>;
}

declare const key: string;
export function ComputedKeyLiteral() {
  return <div>{createComponent(Loading, { get children() { return <p>{a()}</p>; }, [key]: 1 })}</div>;
}

// The primitive behind the boundary re-arms through the same `on` (Chrome: LOADING_ON_OUTSIDE_HOLD after the click): SC5006 at it.
export function PrimitiveOn() {
  const view = createLoadingBoundary(() => <p>{a()}</p>, () => <p>f</p>, { on: () => t() });
  return <div>{view()}<p>{a()}</p></div>;
}

// Stays clean: a local function named `Loading` is not the boundary.
export function ShadowedByLocal() {
  function Loading(props: { on?: any; children?: any }) {
    return <section>{props.children}</section>;
  }
  return (
    <div>
      <Loading on={t()}>
        <p>{a()}</p>
      </Loading>
      {Loading({ on: t, children: <p>x</p> })}
    </div>
  );
}

// Stays clean: a project component with a body, whose own tags are asked where they stand.
function Legend(props: { on?: any; children?: any }) {
  return <section>{props.children}</section>;
}
export function ProjectComponent() {
  return (
    <Legend on={t()}>
      <p>x</p>
    </Legend>
  );
}

// Stays clean: `createLoadingBoundary` is not `Loading`, and takes no `on` here.
export function PrimitiveBoundary() {
  const view = createLoadingBoundary(() => <p>{a()}</p>, () => <p>f</p>);
  return <div>{view()}</div>;
}
