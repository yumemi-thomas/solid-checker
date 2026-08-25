import { createSignal, onCleanup } from "solid-js";

// Ordinary void and <noscript> child lists carry positive compiler facts at the
// current pins: positions the selected compiler keeps are tracked; positions it
// deletes are one elided range. Neither outcome is inferred from silence.
function NestedVoidChild() {
  const [count] = createSignal(0);
  return <div><br>{count()}</br></div>;
}

function RootVoidChildIsDiscarded() {
  const [total] = createSignal(0);
  return <br>{total()}</br>;
}

// Attributes are not child content and remain ordinary censused sites.
function VoidAttributeStaysCertified() {
  const [id] = createSignal("i");
  return <div><br id={id()} /></div>;
}

function AdjacentTrackedChildStaysCertified() {
  const [name] = createSignal("n");
  return <div><br /><span>{name()}</span><hr /></div>;
}

function RootNoscriptChild() {
  const [a] = createSignal(0);
  return <noscript>{a()}</noscript>;
}

function NestedNoscriptOffTheFastPath() {
  const [body] = createSignal(0);
  const [tag] = createSignal("t");
  return <div><noscript id={tag()}>{body()}</noscript></div>;
}

// Dialect-specific controls. Solid 1.x treats these legacy tags as void and
// reports their child lists discarded; Solid 2 treats them as non-void and
// reports their reads tracked. Both outcomes are certified and silent.
function NestedKeygenChild() {
  const [keyed] = createSignal(0);
  return <div><keygen>{keyed()}</keygen></div>;
}

function RootMenuitemChild() {
  const [item] = createSignal(0);
  return <menuitem>{item()}</menuitem>;
}

// The former producer-side exit-2 residue is closed here too. The compiler
// retains the outer elided attribute decision while retracting its unvisited
// nested JSX site, so the deleted read is silent and the visible source child
// remains tracked.
function ShadowedJsxValuedChildrenReconciles() {
  const [hidden] = createSignal(0);
  const [visible] = createSignal(0);
  return <span children={<b>{hidden()}</b>}>{visible()}</span>;
}

// Ownership follows the same positive facts: Solid 1.x discards this range;
// Solid 2 keeps the nested child and reports the insert owner. Both are silent.
export const CleanupInsideDiscardedChild = (
  <div><br>{(onCleanup(() => {}), null)}</br></div>
);

// Censused control: the insert wrapper establishes the owner.
export const CleanupInsideACertifiedChild = (
  <div><span>{(onCleanup(() => {}), null)}</span></div>
);

// Non-JSX control: this remains a proven missing-owner violation.
onCleanup(() => {});

export function Root() {
  return (
    <div>
      <NestedVoidChild />
      <RootVoidChildIsDiscarded />
      <VoidAttributeStaysCertified />
      <AdjacentTrackedChildStaysCertified />
      <RootNoscriptChild />
      <NestedNoscriptOffTheFastPath />
      <NestedKeygenChild />
      <RootMenuitemChild />
      <ShadowedJsxValuedChildrenReconciles />
    </div>
  );
}
