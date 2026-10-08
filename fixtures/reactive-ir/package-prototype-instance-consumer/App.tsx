import { createMemo, createTrackedEffect } from "solid-js";
import { PrototypeSet, PrototypeMap } from "reactive-package";
declare function keep(value: unknown): void;

export function LeafHas() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { console.log(selected.has(1)); });
  return <div />;
}
export function LeafSize() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { console.log(selected.size); });
  return <div />;
}
export function LeafResume() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { console.log(selected.values().next()); });
  return <div />;
}
export function LeafIteration() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { console.log([...selected]); });
  return <div />;
}
export function MapLeafGet() {
  const table = new PrototypeMap<number, string>();
  createTrackedEffect(() => { console.log(table.get(1)); });
  return <div />;
}
export function MapLeafEntries() {
  const table = new PrototypeMap<number, string>();
  createTrackedEffect(() => { console.log(table.entries().next()); });
  return <div />;
}
export function PriorTrackedReadLeaf() {
  const selected = new PrototypeSet<number>();
  const observed = createMemo(() => selected.has(1));
  createTrackedEffect(() => { console.log(selected.has(1)); });
  return <div>{String(observed())}</div>;
}
export function TrackedReads() {
  const selected = new PrototypeSet<number>();
  const table = new PrototypeMap<number, string>();
  return <div>{String(selected.has(1))}{selected.size}{String([...selected])}
    {String([...selected.values()])}{String(table.get(1))}{table.size}{String([...table])}</div>;
}
export function UnobservedReads() {
  const selected = new PrototypeSet<number>();
  const snapshot = selected.has(1);
  const size = selected.size;
  return <div>{String(snapshot)}{size}</div>;
}
export function DiscardedIterator() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { selected.values(); });
  return <div />;
}
export function NestedMemo() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { createMemo(() => selected.has(1)); });
  return <div />;
}
export function ArrayEscape() {
  const selected = new PrototypeSet<number>();
  keep([selected]);
  return <div>{String(selected.has(1))}</div>;
}
export function ShorthandEscape() {
  const selected = new PrototypeSet<number>();
  keep({ selected });
  return <div>{String(selected.has(1))}</div>;
}
export function AliasEscape() {
  const selected = new PrototypeSet<number>();
  const alias = selected;
  return <div>{String(alias.has(1))}</div>;
}
export function PassedEscape() {
  const selected = new PrototypeSet<number>();
  keep(selected);
  return <div />;
}
export function ReturnedEscape() {
  const selected = new PrototypeSet<number>();
  return selected;
}
export function JsxEscape() {
  const selected = new PrototypeSet<number>();
  return <div payload={selected} />;
}
export function CastEscape() {
  const selected = new PrototypeSet<number>();
  keep(selected as Set<number>);
  return <div />;
}
export function WrappedReceiver() {
  const selected = new PrototypeSet<number>();
  return <div>{String((selected as Set<number>).has(1))}</div>;
}
export function WrappedCallee() {
  const selected = new PrototypeSet<number>();
  return <div>{String((selected.has as (key: number) => boolean)(1))}</div>;
}
export function WrappedConstructor() {
  const selected = new (PrototypeSet as typeof PrototypeSet)<number>();
  return <div>{String(selected.has(1))}</div>;
}
export function StoredIterator() {
  const selected = new PrototypeSet<number>();
  const iterator = selected.values();
  return <div>{String(iterator.next())}</div>;
}
export function ComputedMember(key: "has") {
  const selected = new PrototypeSet<number>();
  return <div>{String(selected[key](1))}</div>;
}
export function MemberMutation() {
  const selected = new PrototypeSet<number>();
  selected.has = () => false;
  return <div>{String(selected.has(1))}</div>;
}
export function MutableBinding() {
  let selected = new PrototypeSet<number>();
  return <div>{String(selected.has(1))}</div>;
}
export function InlineInstance() {
  return <div>{String(new PrototypeSet<number>().has(1))}</div>;
}
export function HelperObserverUnknown() {
  const selected = new PrototypeSet<number>();
  function read() { return selected.has(1); }
  return <div>{String(read())}</div>;
}
export function ShadowedConstructor() {
  class PrototypeSet<T> extends Set<T> {}
  const selected = new PrototypeSet<number>();
  return <div>{String(selected.has(1))}</div>;
}

export function DiscardedWrappedConstructor() {
  new (PrototypeSet as typeof PrototypeSet)<number>();
  return <div>wrapped constructor</div>;
}
