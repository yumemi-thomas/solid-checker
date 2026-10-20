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

// A second spread is a `mergeProps` view only when some source is provably
// never falsy. `extra` is a bare identifier that may be `undefined`, so
// `mergeProps(props, extra)` can hand the callee the parent's props object
// itself (rc.13 `merge` returns its one non-falsy source), and the callee may
// delete or redefine the getter before a sibling reads it.
type Wide = { count?: number; label?: string; children?: unknown; extra?: { tag?: string } };
const extra: { tag?: string } | undefined = undefined;
function WipeByDelete(props: Wide) {
  delete props.count;
  return <span />;
}
function WipeByDefine(props: Wide) {
  Object.defineProperty(props, "count", { value: 0 });
  return <span />;
}
function ExtraDeleteReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function ExtraDefineReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
// Uncertifiable: the bare-identifier second spread may be `undefined`.
function ExtraDeleteOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {...extra} />
      <ExtraDeleteReader {...props} />
    </div>
  );
}
// Uncertifiable: the same through `Object.defineProperty`.
function ExtraDefineOuter(props: Wide) {
  return (
    <div>
      <WipeByDefine {...props} {...extra} />
      <ExtraDefineReader {...props} />
    </div>
  );
}

// Violation: a member spread is dynamic, compiled to the function source
// `() => props.extra`, which `merge` never drops: the callee gets a view.
function MemberSpreadReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function MemberSpreadOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {...props.extra} />
      <MemberSpreadReader {...props} />
    </div>
  );
}

// Violation: an attribute is an object-literal source, never falsy.
function AttributeSpreadReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function AttributeSpreadOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {...extra} label="x" />
      <AttributeSpreadReader {...props} />
    </div>
  );
}

// Violation: a rendered child is an object-literal source too.
function ChildSpreadReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function ChildSpreadOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {...extra}>
        <span />
      </WipeByDelete>
      <ChildSpreadReader {...props} />
    </div>
  );
}

// Babel drops a `children` attribute when the element has any raw child node
// (`hasChildren && key === "children"`), even whitespace or `{/* */}`, so the
// element has no never-falsy source and the callee gets the props object.
// Uncertifiable.
function ChildrenAttrReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function ChildrenAttrOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {...extra} children="x">
        {/* nothing */}
      </WipeByDelete>
      <ChildrenAttrReader {...props} />
    </div>
  );
}
// Violation: without a raw child the `children` attribute is a real source.
function ChildrenAttrSelfReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function ChildrenAttrSelfOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {...extra} children="x" />
      <ChildrenAttrSelfReader {...props} />
    </div>
  );
}

// Babel's `isDynamic` reads a leading `/*@static*/` comment and passes the
// member spread raw (`mergeProps(props, props.extra)`), which is `props` when
// `extra` is undefined. Uncertifiable.
function StaticMemberReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function StaticMemberOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props} {.../*@static*/ props.extra} />
      <StaticMemberReader {...props} />
    </div>
  );
}

// Babel's `filterChildren` drops newline-led text that is whitespace by
// JavaScript's `\s`, which includes U+FEFF; the only child below is a newline,
// a U+FEFF and a newline, so the callee gets the props object. Uncertifiable.
function BomReader(props: Wide) {
  const count = props.count;
  return <span>{count}</span>;
}
function BomOuter(props: Wide) {
  return (
    <div>
      <WipeByDelete {...props}>
﻿
      </WipeByDelete>
      <BomReader {...props} />
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
      <ExtraDeleteOuter count={count()} />
      <ExtraDefineOuter count={count()} />
      <MemberSpreadOuter count={count()} />
      <AttributeSpreadOuter count={count()} />
      <ChildSpreadOuter count={count()} />
      <ChildrenAttrOuter count={count()} />
      <ChildrenAttrSelfOuter count={count()} />
      <StaticMemberOuter count={count()} />
      <BomOuter count={count()} />
    </div>
  );
}
