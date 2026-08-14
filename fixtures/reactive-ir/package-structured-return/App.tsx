import { createSignal } from "solid-js";
import { identityTuple, structuredObject, structuredTuple } from "structured-package";

export function TupleConsumer() {
  const [state, value] = structuredTuple();
  const stateValue = state.value;
  const accessorValue = value();
  return <div>{stateValue + accessorValue}</div>;
}

export function ObjectDestructureConsumer() {
  const { active } = structuredObject();
  const activeValue = active();
  return <div>{String(activeValue)}</div>;
}

export function ObjectMemberConsumer() {
  const result = structuredObject();
  const pendingValue = result.pending();
  return <div>{String(pendingValue)}</div>;
}

export function DirectObjectMemberConsumer() {
  const pendingValue = structuredObject().pending();
  return <div>{String(pendingValue)}</div>;
}

export function IdentityWrapperConsumer() {
  const [persisted] = identityTuple(createSignal(1));
  const persistedValue = persisted();
  return <div>{persistedValue}</div>;
}
