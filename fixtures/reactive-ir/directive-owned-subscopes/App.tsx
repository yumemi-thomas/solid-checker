import { createMemo, createRoot, createSignal, getOwner, runWithOwner } from "solid-js";

const [count] = createSignal(0);

function own(fn: () => void) {
  createRoot(dispose => {
    fn();
    dispose();
  });
}

function tip(label: () => number) {
  return (element: HTMLDivElement) => {
    element.title = "tip";
    void label;
  };
}

export function App() {
  const owner = getOwner();
  return (
    <>
      {/* Positive: a memo created in the apply callback itself is unowned. */}
      <div ref={element => { createMemo(() => count()); void element; }} />
      {/* Negative: an eager argument of the ref expression runs while
          rendering, under the component's owner (runtime-probed). */}
      <div ref={tip(createMemo(() => count()))} />
      {/* Negative: a memo inside a root the apply callback creates is owned
          by that root. The createRoot call itself is uncertifiable: its
          allocation alone does not prove an undisposed lifetime (ADR 0355). */}
      <div
        ref={element => {
          createRoot(dispose => {
            createMemo(() => count());
            dispose();
          });
          void element;
        }}
      />
      {/* Uncertifiable: runWithOwner supplies an owner that may be null. */}
      <div ref={element => { runWithOwner(owner, () => createMemo(() => count())); void element; }} />
      {/* Positive, both elements: each function of a ref array is applied. */}
      <div ref={[element => { createMemo(() => count()); void element; }, element => { createMemo(() => count()); void element; }]} />
      {/* Negative: an IIFE building the callback runs while rendering. */}
      <div ref={(() => { createMemo(() => count()); return (element: HTMLDivElement) => { void element; }; })()} />
      {/* Uncertifiable: a callback an IIFE returns may be the applied one. */}
      <div ref={(() => (element: HTMLDivElement) => { createMemo(() => count()); void element; })()} />
      {/* Negative: an IIFE inside a root's callback runs at once, owned. */}
      <div ref={element => { createRoot(() => { (() => createMemo(() => count()))(); }); void element; }} />
      {/* Positive: a default parameter runs when the callback is applied. */}
      <div ref={(element: HTMLDivElement, memo = createMemo(() => count())) => { void element; void memo; }} />
      {/* Negative: a default parameter of a root's callback runs in the root. */}
      <div ref={element => { createRoot((dispose, memo = createMemo(() => count())) => { void memo; dispose(); }); void element; }} />
      {/* Uncertifiable: a project helper may run the callback under an owner. */}
      <div ref={element => { own(() => { createMemo(() => count()); }); void element; }} />
      {/* Uncertifiable: calling a generator runs none of its body. */}
      <div ref={function* (element: HTMLDivElement) { createMemo(() => count()); void element; }} />
      {/* Positive: a timer inside a root's callback runs with no owner. */}
      <div
        ref={element => {
          createRoot(() => {
            setTimeout(() => createMemo(() => count()));
          });
          void element;
        }}
      />
    </>
  );
}
