import { createTrackedEffect } from "solid-js";
import { PrototypeSet } from "reactive-package";
import * as collectionModule from "reactive-package";
declare function keep(value: unknown): void;
keep(collectionModule);

export function NamespaceEscape() {
  const selected = new PrototypeSet<number>();
  createTrackedEffect(() => { console.log(selected.has(1)); });
  return <div>open constructor</div>;
}
