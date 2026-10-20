import { createSignal, merge } from "solid-js";

// Violation: `SpreadOuter` spreads its own props, and its only tag passes a
// signal read for `count`. The merged view reads the parent's getter, so the
// body read in `SpreadInner` is untracked.
function SpreadInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function SpreadOuter(props: { count: number }) {
  return <SpreadInner {...props} />;
}

// Violation: a later attribute of another name leaves `count` to the spread.
function LabelledInner(props: { count: number; label: string }) {
  const count = props.count;
  return <span>{count}{props.label}</span>;
}
function LabelledOuter(props: { count: number }) {
  return <LabelledInner {...props} label="static" />;
}

// Uncertifiable: the attribute after the spread sets `count`, so the spread
// does not supply it; the spread call site still proves nothing static.
function OverriddenInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function OverriddenOuter(props: { count: number }) {
  return <OverriddenInner {...props} count={0} />;
}

// Uncertifiable: the last spread is another object, which may set `count`.
function LaterSpreadInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function LaterSpreadOuter(props: { count: number; extra: { count: number } }) {
  return <LaterSpreadInner {...props} {...props.extra} />;
}

// Uncertifiable: a `merge` view is not the parent's props.
function ViewInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function ViewOuter(props: { count: number }) {
  const view = merge(props, { label: "x" });
  return <ViewInner {...view} />;
}

// Uncertifiable: a reassigned props parameter is no longer the props object.
function ReassignedInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function ReassignedOuter(props: { count: number }) {
  props = { count: 0 };
  return <ReassignedInner {...props} />;
}

// Uncertifiable: every caller of `StaticOuter` passes a literal, so nothing
// reactive is witnessed through the spread.
function StaticInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function StaticOuter(props: { count: number }) {
  return <StaticInner {...props} />;
}

// Violation: a lone spread hands the props object itself to `ChainMiddle`,
// which only spreads it on; the chain keeps the parent's getter.
function ChainInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function ChainMiddle(props: { count: number }) {
  return <ChainInner {...props} />;
}
function ChainOuter(props: { count: number }) {
  return <ChainMiddle {...props} />;
}

// Uncertifiable: `Deleter` receives the same object through a lone spread
// and deletes the getter its sibling would read.
function Deleter(props: { count?: number }) {
  delete props.count;
  return <span />;
}
function SiblingInner(props: { count: number; label: string }) {
  const count = props.count;
  return <span>{count}{props.label}</span>;
}
function DeletingOuter(props: { count: number }) {
  return (
    <div>
      <Deleter {...props} />
      <SiblingInner {...props} label="x" />
    </div>
  );
}

export function App() {
  const [count] = createSignal(1);
  return (
    <div>
      <SpreadOuter count={count()} />
      <LabelledOuter count={count()} />
      <OverriddenOuter count={count()} />
      <LaterSpreadOuter count={count()} extra={{ count: 0 }} />
      <ViewOuter count={count()} />
      <ReassignedOuter count={count()} />
      <StaticOuter count={1} />
      <ChainOuter count={count()} />
      <DeletingOuter count={count()} />
    </div>
  );
}
