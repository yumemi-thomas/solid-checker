import { createRoot, getOwner, onCleanup } from "solid-js";
import * as Solid from "solid-js";
import { getOwner as currentOwner } from "solid-js";

const registry = new Set<string>();

// Clean: `onCleanup` runs only once this call has seen an owner.
export function registerAnd(url: string) {
  registry.add(url);
  getOwner() && onCleanup(() => registry.delete(url));
}

export function registerIf(url: string) {
  registry.add(url);
  if (getOwner()) onCleanup(() => registry.delete(url));
}

export function registerTernary(url: string) {
  registry.add(url);
  getOwner() ? onCleanup(() => registry.delete(url)) : undefined;
}

// Clean: the same through a namespace import and an aliased import.
export function registerNamespace(url: string) {
  registry.add(url);
  Solid.getOwner() && Solid.onCleanup(() => registry.delete(url));
}

export function registerAlias(url: string) {
  registry.add(url);
  currentOwner() && onCleanup(() => registry.delete(url));
}

// Violation: unguarded.
export function registerUnguarded(url: string) {
  registry.add(url);
  onCleanup(() => registry.delete(url));
}

// Violation: the inverted guard runs `onCleanup` exactly when there is none.
export function registerInverted(url: string) {
  registry.add(url);
  getOwner() || onCleanup(() => registry.delete(url));
}

// Every helper is called at module scope, where there is no owner.
registerAnd("/and");
registerIf("/if");
registerTernary("/ternary");
registerNamespace("/namespace");
registerAlias("/alias");
registerUnguarded("/unguarded");
registerInverted("/inverted");

// Under an owner the guarded helper does register its cleanup.
createRoot((dispose) => {
  registerAnd("/owned");
  dispose();
});
