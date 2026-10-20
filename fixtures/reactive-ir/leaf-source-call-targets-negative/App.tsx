import { createTrackedEffect } from "solid-js";
import { makeEscapedService, makeLeakyService, makeMutableService, makeReceiverService, makeService } from "../leaf-source-call-targets/service";
declare function handOff(value: unknown): void;

export function ReceiverMethod() {
  const service = makeReceiverService();
  createTrackedEffect(() => { service.receiverLeaf(); });
  return <div />;
}
export function MutatedMethod() {
  const service = makeMutableService();
  service.mutableLeaf = () => {};
  createTrackedEffect(() => { service.mutableLeaf(); });
  return <div />;
}
export function EscapedMethod() {
  const service = makeEscapedService();
  handOff(service);
  createTrackedEffect(() => { service.escapedLeaf(); });
  return <div />;
}
export function LeakySibling() {
  const service = makeLeakyService();
  handOff(service.leak());
  createTrackedEffect(() => { service.untouchedLeaf(); });
  return <div />;
}

export function ConstructorAssertion() {
  const service = makeService();
  createTrackedEffect(() => {
    void new (service.forbiddenLeaf as unknown as { new(): object })();
  });
  return <div />;
}
