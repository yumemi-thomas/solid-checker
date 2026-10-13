import { createSignal, type Accessor } from "solid-js";
import { makeArrow, makeHanded, makeKept, makePlain, relabel } from "./factories";

function inspect(read: () => string): string {
  return read();
}

// --- Positive: every receiver still holds the literal. -----------------

// No hand-off: SC1001.
export function Kept() {
  const [label] = createSignal("k");
  const tag = { label };
  const v = tag.label();
  return <p>{v}</p>;
}

// Passing the member itself or reading another member cannot replace
// `label`: SC1001 on the direct read.
export function MemberOnly() {
  const [label] = createSignal("m");
  const tag = { label, other: 1 };
  inspect(tag.label);
  const v = tag.label() + tag.other;
  return <p>{v}</p>;
}

// A closed factory, across files: SC1001.
export function FactoryKept() {
  const tag = makeKept();
  const v = tag.label();
  return <p>{v}</p>;
}

// An expression-bodied closed factory: SC1001.
export function ArrowKept() {
  const tag = makeArrow();
  const v = tag.label();
  return <p>{v}</p>;
}

// A member an interface declares, read through a receiver that is no project
// literal: the type-only premise is unchanged, SC1001.
interface Shared {
  label: Accessor<string>;
}
declare const shared: Shared;
export function External() {
  const v = shared.label();
  return <p>{v}</p>;
}

// --- Negative: the object may have had `label` replaced. ---------------

// Argument hand-off: silent.
export function Tagged() {
  const [label] = createSignal("t");
  const tag = { label };
  relabel(tag);
  const v = tag.label();
  return <p>{v}</p>;
}

// Longhand property, argument hand-off: silent.
export function Longhand() {
  const [label] = createSignal("l");
  const tag = { label: label };
  relabel(tag);
  const v = tag.label();
  return <p>{v}</p>;
}

// Factory-internal hand-off: silent.
export function FactoryHanded() {
  const tag = makeHanded();
  const v = tag.label();
  return <p>{v}</p>;
}

// Consumer-side hand-off of a factory's object: silent.
export function Consumer() {
  const tag = makePlain();
  relabel(tag);
  const v = tag.label();
  return <p>{v}</p>;
}

// Alias written through a widened annotation: silent.
export function Alias() {
  const [label] = createSignal("a");
  const tag = { label };
  const other: { label: () => string } = tag;
  other.label = () => "fixed";
  const v = tag.label();
  return <p>{v}</p>;
}

// Write through the receiver itself: silent.
export function Written() {
  const [label] = createSignal("w");
  const tag = { label };
  (tag as { label: () => string }).label = () => "fixed";
  const v = tag.label();
  return <p>{v}</p>;
}

// A method of the literal hands `this` off: silent.
export function Method() {
  const [label] = createSignal("this");
  const tag = {
    label,
    reset() {
      relabel(this);
    },
  };
  tag.reset();
  const v = tag.label();
  return <p>{v}</p>;
}

// A member an interface declares, read through a project literal that is
// handed off: silent.
interface Labelled {
  label: Accessor<string>;
}
export function Declared() {
  const [label] = createSignal("d");
  const tag: Labelled = { label };
  relabel(tag);
  const v = tag.label();
  return <p>{v}</p>;
}
