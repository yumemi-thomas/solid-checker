import { createSignal, merge, omit } from "solid-js";
import { Root } from "reactive-package";

const [count] = createSignal(1);

function Other(props: Record<string, unknown>) {
  return <div>{String(props.label)}</div>;
}

// Clean: an `on…` prop of a package component whose contract runs its event
// members only at an external event, with no `as` that could replace them.
export function Direct() {
  return <Root onClick={() => console.log(count())} />;
}
export function AnotherEvent() {
  return <Root onFocus={() => console.log(count())} />;
}

// Clean: through a project component that spreads an `omit` view of its props.
function Wrap(props: { onClick?: (event: unknown) => void; label?: string }) {
  const others = omit(props, "label");
  return <Root {...others} />;
}
export function Wrapped() {
  return <Wrap onClick={() => console.log(count())} />;
}

// Uncertifiable: `as` is a component, which receives every prop.
export function AsComponent() {
  return <Root as={Other} onClick={() => console.log(count())} />;
}

// Uncertifiable: a spread at the element could carry `as`.
export function SpreadAtOrigin(props: { as?: string }) {
  return <Root {...props} onClick={() => console.log(count())} />;
}

// Uncertifiable: a merge can add `as`.
function MergeWrap(props: { onClick?: (event: unknown) => void }) {
  const merged = merge({ as: "a" }, props);
  return <Root {...merged} />;
}
export function MergeWrapped() {
  return <MergeWrap onClick={() => console.log(count())} />;
}

// Uncertifiable: `render` is not an event member.
export function NotAnEvent() {
  return <Root render={() => count()} />;
}
