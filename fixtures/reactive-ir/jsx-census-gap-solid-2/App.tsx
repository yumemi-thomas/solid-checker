import { createSignal } from "solid-js";

// Compiler silence inside source JSX is an uncertifiable execution fact, not a
// proof that the read is stale and not a certification that it was deleted.
function ReadInsideVoidElementChild() {
  const [count] = createSignal(0);
  return <br>{count()}</br>;
}

// Ryan's next transform keeps its own textContent/children semantics. At the
// current pin body() has no execution census, while SC8003 independently owns
// the visible children-and-textContent conflict.
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
