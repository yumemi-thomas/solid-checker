import { children, createEffect, createTrackedEffect, omit, Show } from "solid-js";

// ADR 1031: a component element's children (or a prop's value) are a getter that runs where the component reads the prop. Where every read of that prop in the
// project component is on the component's own stack under its render's owner (in its body, in the JSX of an intrinsic element, in a `children()` helper's
// function) and the props object never escapes, the getter runs there, and its creations are answered as code written at those reads.

function Inner() {
  return <p>inner</p>;
}

// Placed (silent): the read is in an intrinsic element's JSX.
function InJsx(props: { children?: any }) {
  return <p>{props.children}</p>;
}
// Placed: the read is in the body.
function InBody(props: { children?: any }) {
  const held = props.children;
  return <p>{held}</p>;
}
// Placed: the read is in the function of a `children()` helper (its memo, under the component's owner).
function ViaHelper(props: { children?: any }) {
  const resolved = children(() => props.children);
  return <p>{resolved()}</p>;
}
// Placed: a prop's value, read in JSX, and no other prop of the object escapes.
function SlotInJsx(props: { slot?: any; label?: string }) {
  return (
    <p>
      {props.label}
      {props.slot}
    </p>
  );
}

// Placed: read directly in the children of `Show`, an in-place renderer, which reads them under its own render's owner.
function ThroughShow(props: { children?: any }) {
  return <Show when={1}>{props.children}</Show>;
}
// Placed: `omit`'s view spread on an intrinsic element reads every prop it does not omit, `children` among them, in that element's render.
function Spreads(props: { children?: any; class?: string }) {
  const others = omit(props, "class");
  return <div {...others} />;
}
// Not placed (`getter-runs-where-read`): the counterexample, a read on an effect's apply (Chrome rc.21: PRIMITIVE_IN_EFFECT_CALLBACK at the tag).
function OnApply(props: { children?: any }) {
  createEffect(
    () => 0,
    () => {
      void props.children;
    },
  );
  return <p>{props.children}</p>;
}
// A leaf owner's callback reads it (PRIMITIVE_IN_FORBIDDEN_SCOPE).
function InLeaf(props: { children?: any }) {
  createTrackedEffect(() => {
    void props.children;
  });
  return <p>leaf</p>;
}
// A handler reads it: a nested function, run where it is called.
function InHandler(props: { children?: any }) {
  return <button onClick={() => void props.children}>go</button>;
}
// The props object escapes (handed whole to a function nothing here reads).
function Escapes(props: { children?: any }) {
  keep(props);
  return <p>{props.children}</p>;
}
function keep(_value: unknown) {}
// Destructured in the parameter: no props object whose reads are known.
function Destructured({ children: kids }: { children?: any }) {
  return <p>{kids}</p>;
}
// `children` omitted and read nowhere else: never run (no placement).
function SpreadOmits(props: { children?: any; class?: string }) {
  const others = omit(props, "children");
  return <div {...others} />;
}
// The view escapes as well as being spread.
function SpreadEscapes(props: { children?: any; class?: string }) {
  const others = omit(props, "class");
  keep(others);
  return <div {...others} />;
}
// Never read: the getter never runs (ADR 1021's ruling: no placement for code that never runs).
function Ignores(_props: { children?: any }) {
  return <p>ignores</p>;
}

export function ByJsx() {
  return (
    <p>
      <InJsx>
        <Inner />
      </InJsx>
    </p>
  );
}

export function ByBody() {
  return (
    <p>
      <InBody>
        <Inner />
      </InBody>
    </p>
  );
}

export function ByHelper() {
  return (
    <p>
      <ViaHelper>
        <Inner />
      </ViaHelper>
    </p>
  );
}

export function BySlot() {
  return (
    <p>
      <SlotInJsx slot={<Inner />} label="x" />
    </p>
  );
}

export function ByApply() {
  return (
    <p>
      <OnApply>
        <Inner />
      </OnApply>
    </p>
  );
}

export function ByLeaf() {
  return (
    <p>
      <InLeaf>
        <Inner />
      </InLeaf>
    </p>
  );
}

export function ByHandler() {
  return (
    <p>
      <InHandler>
        <Inner />
      </InHandler>
    </p>
  );
}

export function ByEscape() {
  return (
    <p>
      <Escapes>
        <Inner />
      </Escapes>
    </p>
  );
}

export function ByShow() {
  return (
    <p>
      <ThroughShow>
        <Inner />
      </ThroughShow>
    </p>
  );
}

export function ByDestructured() {
  return (
    <p>
      <Destructured>
        <Inner />
      </Destructured>
    </p>
  );
}

export function ByIgnores() {
  return (
    <p>
      <Ignores>
        <Inner />
      </Ignores>
    </p>
  );
}

export function BySpread() {
  return (
    <p>
      <Spreads>
        <Inner />
      </Spreads>
    </p>
  );
}

export function BySpreadOmits() {
  return (
    <p>
      <SpreadOmits>
        <Inner />
      </SpreadOmits>
    </p>
  );
}

export function BySpreadEscapes() {
  return (
    <p>
      <SpreadEscapes>
        <Inner />
      </SpreadEscapes>
    </p>
  );
}
