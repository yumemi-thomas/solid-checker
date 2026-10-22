import { createEffect, createSignal, untrack } from "solid-js";

const [count] = createSignal(0);

// A project helper a ref factory hands its callback to: the helper may run the
// callback under an owner of its own, or with none.
function own(run: () => void): (element: HTMLDivElement) => void {
  run();
  return element => { void element; };
}

// A factory whose body creates an effect. A call of it in a ref expression
// runs that body while the component renders, under the component's owner.
function tracked(read: () => number): (element: HTMLDivElement) => void {
  createEffect(() => read(), () => {});
  return element => { void element; };
}

export function App() {
  return (
    <>
      {/* Positive: an effect created in the apply callback itself runs with no owner. */}
      <div ref={element => { createEffect(() => count(), () => { void element; }); }} />
      {/* Negative: an IIFE in the ref expression runs while the component renders. */}
      <div ref={(() => { createEffect(() => count(), () => {}); return (element: HTMLDivElement) => { void element; }; })()} />
      {/* Negative: a factory the ref expression calls runs its body while rendering. */}
      <div ref={tracked(count)} />
      {/* Negative: an untrack callback runs during its call, under the component's owner. */}
      <div ref={untrack(() => { createEffect(() => count(), () => {}); return (element: HTMLDivElement) => { void element; }; })} />
      {/* Negative, then positive: the IIFE element renders; the arrow element is the applied callback. */}
      <div
        ref={[
          (() => { createEffect(() => count(), () => {}); return (element: HTMLDivElement) => { void element; }; })(),
          element => { createEffect(() => count(), () => { void element; }); },
        ]}
      />
      {/* Uncertifiable: calling a generator runs none of its body, and the ref never iterates it. */}
      <div ref={function* (element: HTMLDivElement) { createEffect(() => count(), () => { void element; }); }} />
      {/* Uncertifiable: the project helper may run the callback under an owner. */}
      <div ref={own(() => { createEffect(() => count(), () => {}); })} />
    </>
  );
}

// Positive: at module scope the IIFE runs with no owner, whatever the ref does.
export const detached = <div ref={(() => { createEffect(() => count(), () => {}); return (element: HTMLDivElement) => { void element; }; })()} />;

// Uncertifiable: the getter's body runs when the expression reads `.x`, not when the ref applies the value.
export const viaGetter = <div ref={({ get x() { createEffect(() => count(), () => {}); return (element: HTMLDivElement) => { void element; }; } }).x as any} />;

// Uncertifiable: the tag calls its substitution while the expression evaluates.
function tag(strings: TemplateStringsArray, run: () => void): (element: HTMLDivElement) => void { void strings; run(); return element => { void element; }; }
export const viaTemplate = <div ref={tag`${() => { createEffect(() => count(), () => {}); }}`} />;

// Uncertifiable: an operand of void is never called; the comma yields the other operand.
const noop = (element: HTMLDivElement) => { void element; };
export const viaVoid = <div ref={(void (() => { createEffect(() => count(), () => {}); }), noop)} />;

// Negative: a default parameter initializer runs in its function's call, under the caller's owner (Host renders it).
function Holder(scope = <div ref={(() => { createEffect(() => count(), () => {}); return noop; })()} />) { return scope; }
export function Host() {
  return <>{Holder()}</>;
}

// Positive: a logical operand is a value the ref application receives, applied with no owner.
export function Logical() {
  const on = true;
  return <div ref={(on && (element => { createEffect(() => count(), () => { void element; }); })) || undefined} />;
}
