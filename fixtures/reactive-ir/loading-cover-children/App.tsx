import { createMemo, Loading } from "solid-js";
import { render } from "@solidjs/web";

const articles = createMemo(async () => {
  const response = await fetch("/api/articles");
  return (await response.json()) as { id: number }[];
});

// Positive (stays covered): the wrapper renders its own `props.children` inside
// its `<Loading>`, so the leaf it wraps is read under a boundary.
function RealWrapper(props: { children: JSX.Element }) {
  return (
    <div>
      <Loading fallback={<div />}>{props.children}</Loading>
    </div>
  );
}

function RealLeaf() {
  return <div>{articles().length}</div>;
}

function RealPage() {
  return (
    <RealWrapper>
      <RealLeaf />
    </RealWrapper>
  );
}

render(() => <RealPage />, document.body);

// Positive (stays covered): `p` is an exact `const` alias of the props
// parameter, so `p.children` is the same value.
function AliasWrapper(props: { children: JSX.Element }) {
  const p = props;
  return (
    <div>
      <Loading fallback={<div />}>{p.children}</Loading>
    </div>
  );
}

function AliasLeaf() {
  return <div>{articles().length}</div>;
}

function AliasPage() {
  return (
    <AliasWrapper>
      <AliasLeaf />
    </AliasWrapper>
  );
}

render(() => <AliasPage />, document.body);

// Uncertifiable (was certified clean): the `<Loading>` holds the `children` of
// another object. The real children render outside it, and Solid warns that the
// leaf's pending read has no boundary. A member spelled `children` is not the
// component's: nothing proves this wrapper covers its children, never a
// violation, because the walk does not follow what the wrapper renders.
function ForeignWrapper(props: { children: JSX.Element }) {
  const other = { children: 1 };
  return (
    <div>
      {props.children}
      <Loading fallback={<div />}>
        <span>{other.children}</span>
      </Loading>
    </div>
  );
}

function ForeignLeaf() {
  return <div>{articles().length}</div>;
}

function ForeignPage() {
  return (
    <ForeignWrapper>
      <ForeignLeaf />
    </ForeignWrapper>
  );
}

render(() => <ForeignPage />, document.body);

// Uncertifiable (the same): an alias of that other object is no alias of the
// props parameter.
function ForeignAliasWrapper(props: { children: JSX.Element }) {
  const other = { children: 1 };
  const o = other;
  return (
    <div>
      {props.children}
      <Loading fallback={<div />}>
        <span>{o.children}</span>
      </Loading>
    </div>
  );
}

function ForeignAliasLeaf() {
  return <div>{articles().length}</div>;
}

function ForeignAliasPage() {
  return (
    <ForeignAliasWrapper>
      <ForeignAliasLeaf />
    </ForeignAliasWrapper>
  );
}

render(() => <ForeignAliasPage />, document.body);

// Uncertifiable (control, unchanged): the same wrapper with the other object's
// member spelled `label`. The spelling alone moved the answer before.
function OutsideWrapper(props: { children: JSX.Element }) {
  const other = { label: 1 };
  return (
    <div>
      {props.children}
      <Loading fallback={<div />}>
        <span>{other.label}</span>
      </Loading>
    </div>
  );
}

function OutsideLeaf() {
  return <div>{articles().length}</div>;
}

function OutsidePage() {
  return (
    <OutsideWrapper>
      <OutsideLeaf />
    </OutsideWrapper>
  );
}

render(() => <OutsidePage />, document.body);

// Uncertifiable (was certified): `props` in the boundary is the parameter of an
// inner function, not the component's. The binder says which; the spelling
// does not.
function ShadowWrapper(props: { children: JSX.Element }) {
  return (
    <div>
      <Loading fallback={<div />}>{[1].map((props: any) => props.children)}</Loading>
    </div>
  );
}

function ShadowLeaf() {
  return <div>{articles().length}</div>;
}

function ShadowPage() {
  return (
    <ShadowWrapper>
      <ShadowLeaf />
    </ShadowWrapper>
  );
}

render(() => <ShadowPage />, document.body);

// Uncertifiable (was certified): the props parameter is reassigned, so
// `props.children` is no longer what the caller passed.
function ReassignedWrapper(props: { children: JSX.Element }) {
  props = { children: <i /> };
  return (
    <div>
      <Loading fallback={<div />}>{props.children}</Loading>
    </div>
  );
}

function ReassignedLeaf() {
  return <div>{articles().length}</div>;
}

function ReassignedPage() {
  return (
    <ReassignedWrapper>
      <ReassignedLeaf />
    </ReassignedWrapper>
  );
}

render(() => <ReassignedPage />, document.body);
