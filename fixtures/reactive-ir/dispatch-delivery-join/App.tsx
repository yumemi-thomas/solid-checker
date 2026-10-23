import { createSignal, onCleanup } from "solid-js";

// Every function here dispatches only a supported event (non-bubbling,
// non-delegated, `new Event`/`new CustomEvent` of a literal type, on a local
// const element), so the delivery set of each site is exactly the dispatched
// element's own handlers for that type (ADR 0361). A site the analysis cannot
// join makes every handler in the program conditional, so each such shape is a
// project of its own under variants/.

// The scroll handler runs on the dispatcher's stack, under the component's
// owner: its `onCleanup` stays an obligation. The click handler on another
// element is reached by no dispatch: a proven SC4001 violation.
export function Targeted() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}

// The same through an exact `{...handlers}` spread holder (ADR 0358).
export function TargetedSpread() {
  const handlers = { onScroll: () => { onCleanup(() => undefined); } };
  const el = (<div {...handlers} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}

// A `CustomEvent` whose init states `bubbles: false` and `composed: false`.
export function TargetedCustom() {
  const el = (<div onPing={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new CustomEvent("ping", { detail: 1, bubbles: false, composed: false }));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}

// The dispatched element's handler for another type is not reached by a
// scroll: its click handler is a proven violation as well.
export function OtherTypeOnTheSameElement() {
  const el = (
    <div
      onScroll={() => { onCleanup(() => undefined); }}
      onClick={() => { onCleanup(() => undefined); }}
    />
  ) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <p>other type</p>;
}

// Parentheses, `satisfies` and `!` on the receiver and the event are read
// through.
export function Wrapped() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} /> as HTMLDivElement)!;
  (el!).dispatchEvent((new Event("scroll")) satisfies Event);
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}

// SC2001: a signal write in an unrelated handler is legal where the handler is
// proven ownerless; the dispatched handler's write stays an obligation.
export function WriteTwin() {
  const [count, setCount] = createSignal(0);
  const el = (<div onScroll={() => { setCount(1); }} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { setCount(count() + 1); }}>unrelated</button>;
}

function Child(props: { onScroll: () => void }) {
  const el = (<div onScroll={() => props.onScroll()} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <p>child</p>;
}

// A literal handed to a component and run by its dispatched handler runs under
// the dispatcher's owner wherever it is written, here inside an unrelated
// handler: only code written directly in an element's own handler is joined, so
// this write stays an obligation.
export function NestedPropLiteral() {
  const [count, setCount] = createSignal(0);
  return (
    <button
      onClick={() => {
        const view = <Child onScroll={() => { setCount(count() + 1); }} />;
        void view;
      }}
    >
      unrelated
    </button>
  );
}

function shared() {
  onCleanup(() => undefined);
}

// A function that is also an unrelated element's handler keeps its violation:
// that element's own events run it with no owner, whatever else reaches it.
export function SharedHandler() {
  return (
    <div>
      <Child onScroll={shared} />
      <button onClick={shared}>unrelated</button>
    </div>
  );
}

// A project method that shares a dispatching member's spelling resolves to its
// own declaration, so it is no dispatch: the unrelated handler keeps its proof.
const project = { click() { return 1; }, focus() { return 2; } };
export function ProjectMethods() {
  project.click();
  project.focus();
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
