import { createOptimistic } from "solid-js";
import { pair, record } from "reactive-package";
import * as carriers from "reactive-package";

export function BadTuple() {
  const [source] = createOptimistic(0);
  const [value] = pair<() => number>(source);
  const stale = value();
  return <div>{stale}</div>;
}
export function GoodObject() {
  const [source] = createOptimistic(0);
  const result = record<() => number>(source);
  return <div>{result.value()}</div>;
}
export function BadObject() {
  const [source] = createOptimistic(0);
  const result = record<() => number>(source);
  const stale = result.value();
  return <div>{stale}</div>;
}
export function ReboundAccessor() {
  const [source] = createOptimistic(0);
  let [value] = pair<() => number>(source);
  value = () => 1;
  const plain = value();
  return <div>{plain}</div>;
}
export function ReplacedTupleMember() {
  const [source] = createOptimistic(0);
  const result = pair<() => number>(source);
  result[0] = () => 1;
  const value = result[0]();
  return <div>{value}</div>;
}
export function ReplacedObjectThroughAlias() {
  const [source] = createOptimistic(0);
  const result = record<() => number>(source);
  const alias = result;
  alias.value = () => 1;
  const value = result.value();
  return <div>{value}</div>;
}
export function UnknownIndex(key: 0 | 1) {
  const [source] = createOptimistic(0);
  const result = pair<() => number>(source);
  const value = result[key];
  return <div>{typeof value === "function" ? value() : value}</div>;
}
export function BadNamespace() {
  const [source] = createOptimistic(0);
  const [value] = carriers.pair<() => number>(source);
  const stale = value();
  return <div>{stale}</div>;
}
export function DeletedMember() {
  const [source] = createOptimistic(0);
  const result = record<() => number>(source);
  delete (result as {value?: () => number}).value;
  const value = result.value();
  return <div>{value}</div>;
}
declare function replace(value: unknown): void;
export function DeletedBrandedMember() {
  const [source] = createOptimistic(0);
  const result = record(source);
  delete (result as {value?: () => number}).value;
  const value = result.value();
  return <div>{value}</div>;
}
export function EscapedContainer() {
  const [source] = createOptimistic(0);
  const result = record<() => number>(source);
  replace(result);
  const value = result.value();
  return <div>{value}</div>;
}
