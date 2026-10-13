import { createTrackedEffect } from "solid-js";
import { InputSet, InputMap } from "reactive-package";

export function ArraySetLeaf() {
  const selected = new InputSet<number>([1]);
  createTrackedEffect(() => { console.log(selected.has(1)); });
  return <div />;
}
export function ArrayEntryMapLeaf() {
  const table = new InputMap<number, string>([[1, "one"]]);
  createTrackedEffect(() => { console.log(table.get(1)); });
  return <div />;
}
export function LiteralTracked() {
  const selected = new InputSet<number>([1]);
  return <div>{String(selected.has(1))}</div>;
}
export function NullPopulation() {
  const selected = new InputSet<number>(null);
  return <div>{String(selected.has(1))}</div>;
}
export function DynamicSet(values: Iterable<number>) {
  const selected = new InputSet<number>(values);
  createTrackedEffect(() => { console.log(selected.has(1)); });
  return <div />;
}
export function DynamicMapEntry(entry: readonly [number, string]) {
  const table = new InputMap<number, string>([entry]);
  createTrackedEffect(() => { console.log(table.get(1)); });
  return <div />;
}
export function AliasInput() {
  const values = [1];
  const selected = new InputSet<number>(values);
  return <div>{String(selected.has(1))}</div>;
}
export function SpreadInput(values: number[]) {
  const selected = new InputSet<number>([...values]);
  return <div>{String(selected.has(1))}</div>;
}
