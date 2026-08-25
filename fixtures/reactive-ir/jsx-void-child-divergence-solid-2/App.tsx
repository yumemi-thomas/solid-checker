import { createSignal, onCleanup } from "solid-js";

// At the current compiler pins, ordinary HTML void children and <noscript>
// children are no longer transform divergences. When their source expressions
// survive without an execution entry, the checker treats them as census gaps:
// compiler silence cannot prove either that the expression runs untracked or
// that it was deleted.
function NestedVoidChild() {
  const [count] = createSignal(0);
  return <div><br>{count()}</br></div>;
}

function RootVoidChildDependsOnTheProducer() {
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

// The one surviving transform divergence is dialect-specific. Solid 1.x Babel
// treats keygen and menuitem as void, while the 1.x Rust producer lowers their
// children. Solid 2 treats both as non-void in producer and parity target.
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

// Ownership must fail closed over the same uncertainty. If the producer emits
// no census for this source child, absence of an owner region cannot establish
// that a live unowned cleanup exists.
export const CleanupInsideADivergentChild = (
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
      <RootVoidChildDependsOnTheProducer />
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
