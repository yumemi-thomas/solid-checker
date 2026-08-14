// A shorthand property (`{ tracked }`) writes one identifier where a key and
// a value both stand, and TypeScript answers a symbol query at that span with
// the *property's* symbol. The value binding is named only by the binder's
// resolution of that reference, which is scope-exact. Every export here pairs
// a shorthand with a same-spelled declaration the shorthand cannot see.
import { createMemo } from "solid-js";
import { helper, importedTracked } from "./values";

// Same spelling as the block-scoped accessor in `unprovenShorthand`, and not
// reactive. Nothing may promote it.
const plain = () => "/plain";

export function scopedShorthand() {
  {
    // A sibling block. Invisible at the shorthand below, so it must neither
    // be chosen nor make the visible declaration ambiguous.
    const tracked = createMemo(() => "/decoy");
    void tracked;
  }
  const tracked = createMemo(() => "/users/1");
  return { tracked };
}

export function unprovenShorthand() {
  {
    // The only accessor spelled `plain` in this function, and out of scope at
    // the shorthand. The shorthand names the module-scope plain function, so
    // no reactive claim is provable here.
    const plain = createMemo(() => "/decoy");
    void plain;
  }
  return { plain };
}

export function shadowedShorthand(tracked: () => string) {
  // The parameter shadows nothing reactive; the module has no `tracked`.
  // The accessor of that spelling lives in `scopedShorthand`, out of scope.
  return { tracked };
}

export function importedShorthand() {
  // Resolves to an import specifier, which declares no local accessor: no
  // claim.
  return { helper };
}

export function importedAccessorShorthand() {
  // `importedTracked` *is* an accessor, but it is declared in another file
  // and the binder resolves this reference to the import specifier here.
  // Fails closed rather than matching the declaration by spelling; recorded
  // in docs/precision-backlog.md.
  return { importedTracked };
}

export function globalShorthand() {
  // Resolves to no declaration in this file's scope tree: no claim.
  return { structuredClone };
}

export function writtenShorthand() {
  const tracked = createMemo(() => "/users/2");
  // Not a shorthand at all; the ordinary value path proves this one.
  return { tracked: tracked };
}
