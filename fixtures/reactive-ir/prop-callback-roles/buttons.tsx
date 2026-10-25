import { createMemo, createTrackedEffect, merge } from "solid-js";

type Press = { onPress?: () => void; label?: string };
type Select = { onSelect: (value: unknown) => void };

// ---- The prop reaches only a DOM event: the literal runs when the event is
// dispatched, with no owner and no strict-read window.

// Handed to an intrinsic element's handler.
export function Direct(props: Press) {
  return <button onClick={props.onPress}>{props.label}</button>;
}

// Called inside the element's own handler.
export function InHandler(props: Press) {
  return <button onClick={() => props.onPress?.()}>{props.label}</button>;
}

// Forwarded to a component that hands it to a handler (bounded recursion).
export function Forwards(props: Press) {
  return <Direct onPress={props.onPress} label={props.label} />;
}

// Read through a merge view the dialect proves inert (ADR 0337), then handed
// to a handler.
export function MergedEvent(props: Press) {
  const p = merge({ label: "merged" }, props);
  return <button onClick={p.onPress}>{p.label}</button>;
}

// ---- The prop is called where an owner is current: the literal runs there.

// Called in the component's body.
export function Selects(props: Select) {
  props.onSelect(1);
  return <main />;
}

// Forwarded to a component that calls it in its body.
export function ForwardsToSelects(props: Select) {
  return <Selects onSelect={props.onSelect} />;
}

// A handler and a call in the body: one owned entry decides (ADR 0378).
export function Both(props: Select) {
  props.onSelect(2);
  return <button onClick={props.onSelect}>both</button>;
}

// Called in a tracked computation.
export function InMemo(props: Select) {
  const result = createMemo(() => {
    props.onSelect(3);
    return 0;
  });
  return <main>{result()}</main>;
}

// Called in a leaf scope, where the runtime's write guard exempts a write.
export function InLeaf(props: Select) {
  createTrackedEffect(() => {
    props.onSelect(4);
  });
  return <main />;
}

// A `const` alias of the prop is the same function.
export function Aliased(props: Select) {
  const go = props.onSelect;
  go(5);
  return <main />;
}

// ---- Nothing proves where the prop is called.

// Destructured: the binding is no `props.<name>` use this proof follows.
export function Destructures({ onPress, label }: Press) {
  return <button onClick={onPress}>{label}</button>;
}

// Spread onto a component: it receives every prop, whatever it does with them.
export function SpreadsToComponent(props: Select) {
  return <Selects {...props} />;
}

// Stored where any code may call it later.
const registry: Array<(value: unknown) => void> = [];
export function Stores(props: Select) {
  registry.push(props.onSelect);
  return <main />;
}

// A `let` the body reassigns names two functions.
export function LetAlias(props: Select) {
  let go = props.onSelect;
  go = () => {};
  go(6);
  return <main />;
}

// A merge view the body calls through: only the event-only proof follows views.
export function MergedBody(props: Select) {
  const p = merge({ label: "merged" }, props);
  p.onSelect(7);
  return <main />;
}
