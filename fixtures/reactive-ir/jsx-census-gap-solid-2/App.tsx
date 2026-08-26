import { createSignal } from "solid-js";

// Positive deletion control. At a template root Ryan's next transform discards
// the void child list, and the current trace records that list as Elided.
function ReadInsideVoidElementChild() {
  const [count] = createSignal(0);
  return <br>{count()}</br>;
}

// Current Solid gives source children precedence over textContent. body() is a
// positively tracked insert; SC8003 independently owns the visible
// children-and-textContent authoring conflict.
function TextContentChildNowCertified() {
  const [label] = createSignal("l");
  const [body] = createSignal("b");
  return <div><span textContent={label()}>{body()}</span></div>;
}

// Static noscript lowering leaves no usable execution entry for this source
// child, so it exercises the same fail-closed census-gap path.
function RetractedInertNoscriptChild() {
  const [note] = createSignal("n");
  return <div><noscript>{note()}</noscript></div>;
}

// Censused control.
function TrackedChildStaysCertified() {
  const [name] = createSignal("n");
  return <span>{name()}</span>;
}

// Non-JSX control: the component-body read is proven untracked without relying
// on compiler silence.
function ReadOutsideJsxStaysAViolation() {
  const [total] = createSignal(0);
  const snapshot = total();
  return <span>{snapshot}</span>;
}

export function Root() {
  return (
    <div>
      <ReadInsideVoidElementChild />
      <TextContentChildNowCertified />
      <RetractedInertNoscriptChild />
      <TrackedChildStaysCertified />
      <ReadOutsideJsxStaysAViolation />
    </div>
  );
}
