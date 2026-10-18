import { createSignal } from "solid-js";
import { Anchor, Button, CallingRoot, GetterRoot, IconButton, type PolymorphicProps } from "./polymorphic";
import { ShadowRoot } from "./shadow";

// ---- Clean: every hop keeps `as` a string literal, so `Dynamic` renders an
// intrinsic element and `onClick` reaches only its event listener.

export function UsesButton() {
  const [count] = createSignal(0);
  return <Button onClick={() => console.log(count())}>a</Button>;
}
export function UsesIconButton() {
  const [count] = createSignal(0);
  return <IconButton label="b" onClick={() => console.log(count())} />;
}
export function UsesButtonAsAnchor() {
  const [count] = createSignal(0);
  return <Button as="a" onClick={() => console.log(count())}>c</Button>;
}
export function UsesAnchor(props: PolymorphicProps) {
  const [count] = createSignal(0);
  return <Anchor {...props} onClick={() => console.log(count())} />;
}

// ---- Uncertifiable: the element a hop renders is not proven a string.

function Custom(props: PolymorphicProps) {
  return <i>{String(props.children)}</i>;
}
export function UsesButtonAsComponent() {
  const [count] = createSignal(0);
  return <Button as={Custom} onClick={() => console.log(count())}>d</Button>;
}
export function UsesButtonAsComputed() {
  const [count] = createSignal(0);
  const [tag] = createSignal("a");
  return <Button as={tag()} onClick={() => console.log(count())}>e</Button>;
}
export function UsesButtonSpread(props: PolymorphicProps) {
  const [count] = createSignal(0);
  return <Button {...props} onClick={() => console.log(count())}>f</Button>;
}
export function UsesGetterRoot() {
  const [count] = createSignal(0);
  return <GetterRoot onClick={() => console.log(count())} />;
}
export function UsesShadowRoot() {
  const [count] = createSignal(0);
  return <ShadowRoot onClick={() => console.log(count())} />;
}
// Not an `on…` prop: the element assigns it as a property, which proves
// nothing about whether anything calls it.
export function UsesTooltip() {
  const [count] = createSignal(0);
  return <Button tooltip={() => String(count())}>g</Button>;
}

// ---- Violation: the consumer calls the prop while it renders (ADR 0204).

export function UsesCallingRoot() {
  const [count] = createSignal(0);
  return <CallingRoot onClick={() => console.log(count())} />;
}
