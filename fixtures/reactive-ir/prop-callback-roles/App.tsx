import { createSignal } from "solid-js";
import { Opaque } from "external-ui";
import {
  Aliased, Both, Destructures, Direct, Forwards, ForwardsToSelects, InHandler, InLeaf, InMemo,
  LetAlias, MergedBody, MergedEvent, Selects, SpreadsToComponent, Stores,
} from "./buttons";

// Each caller hands the child a literal that reads and writes a signal. What
// the literal's role is depends only on what the child does with the prop.

// ---- Clean: the child reaches the prop only through a DOM event.

export function UsesDirect() {
  const [open, setOpen] = createSignal(false);
  return <Direct onPress={() => setOpen(!open())} />;
}
export function UsesInHandler() {
  const [open, setOpen] = createSignal(false);
  return <InHandler onPress={() => setOpen(!open())} />;
}
export function UsesForwards() {
  const [open, setOpen] = createSignal(false);
  return <Forwards onPress={() => setOpen(!open())} />;
}
export function UsesMergedEvent() {
  const [open, setOpen] = createSignal(false);
  return <MergedEvent onPress={() => setOpen(!open())} />;
}
// A named function handed to such a child is the same literal.
export function NamedToDirect() {
  const [, setOpen] = createSignal(false);
  const reset = () => setOpen(false);
  return <Direct onPress={reset} />;
}

// ---- The child calls the prop in an owned scope: the write is SC2001 (and a
// read in the body's window is SC1001).

export function UsesSelects() {
  const [open, setOpen] = createSignal(false);
  return <Selects onSelect={() => setOpen(!open())} />;
}
export function UsesForwardsToSelects() {
  const [open, setOpen] = createSignal(false);
  return <ForwardsToSelects onSelect={() => setOpen(!open())} />;
}
export function UsesBoth() {
  const [open, setOpen] = createSignal(false);
  return <Both onSelect={() => setOpen(!open())} />;
}
export function UsesInMemo() {
  const [open, setOpen] = createSignal(false);
  return <InMemo onSelect={() => setOpen(!open())} />;
}
export function UsesAliased() {
  const [open, setOpen] = createSignal(false);
  return <Aliased onSelect={() => setOpen(!open())} />;
}
export function NamedToSelects() {
  const [, setOpen] = createSignal(false);
  const reset = () => setOpen(false);
  return <Selects onSelect={reset} />;
}
// The same function is also a handler: the owned entry still decides.
export function NamedAlsoHandler() {
  const [, setOpen] = createSignal(false);
  const reset = () => setOpen(false);
  return (
    <>
      <button onClick={reset}>reset</button>
      <Selects onSelect={reset} />
    </>
  );
}

// ---- Clean again: a leaf scope exempts the write.

export function UsesInLeaf() {
  const [open, setOpen] = createSignal(false);
  return <InLeaf onSelect={() => setOpen(!open())} />;
}

// ---- Obligations: the child's use of the prop is not one this proof follows.

export function UsesDestructures() {
  const [open, setOpen] = createSignal(false);
  return <Destructures onPress={() => setOpen(!open())} />;
}
export function UsesSpreadsToComponent() {
  const [open, setOpen] = createSignal(false);
  return <SpreadsToComponent onSelect={() => setOpen(!open())} />;
}
export function UsesStores() {
  const [open, setOpen] = createSignal(false);
  return <Stores onSelect={() => setOpen(!open())} />;
}
export function UsesLetAlias() {
  const [open, setOpen] = createSignal(false);
  return <LetAlias onSelect={() => setOpen(!open())} />;
}
export function UsesMergedBody() {
  const [open, setOpen] = createSignal(false);
  return <MergedBody onSelect={() => setOpen(!open())} />;
}
export function UsesOpaque() {
  const [open, setOpen] = createSignal(false);
  return <Opaque onSelect={() => setOpen(!open())} />;
}
