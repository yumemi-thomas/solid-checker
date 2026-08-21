// Solid 1.x, pinned by node_modules/solid-js/package.json.
//
// Every case here failed to be reported, or was reported wrongly, before
// SC7005 stopped asking the primitive vocabulary where a name lives. The
// vocabulary admits a name only when the checker models a reactive obligation
// for it; import location is a different question and has its own generated
// index.

// The one case the old rule did catch: a name in the vocabulary whose module
// differs. It is also the only shape a hardcoded `_ => "solid-js"` fallback
// could get right.
import { createStore } from "solid-js";

// Not in the vocabulary -- Portal carries no reactive obligation, so the rule
// could not see it. Ten of 1.x's `solid-js/web` names were invisible this way,
// and adding them to the vocabulary would have made the fallback report them
// backwards.
import { Portal } from "solid-js";

// Exported by BOTH `solid-js` and `solid-js/web`: 1.x's web entry re-exports
// nine control-flow components. Correct, and silent. A single-module index
// has to pick one of the two and is wrong about the other half of the time;
// upstream's `imports` rule flags this one as a style preference.
import { Show } from "solid-js/web";

// Type position. `Store` is a 1.x type and no value at all, so a value-only
// index cannot see it -- and the old rule skipped type-only imports outright
// on the grounds that they erase. They erase; they still have to resolve.
// `Accessor` beside it is correct and stays silent, which is what keeps this
// from passing by reporting the whole line.
//
// This is also the fix's third shape: the file has no *type-only* declaration
// of `solid-js/store` to merge into, so the fix writes one. Merging into the
// value-position declaration below would compile, and would quietly move a
// type across the erasure boundary.
import type { Store, Accessor } from "solid-js";

// The fix's first shape: one binding, so rewrite the module string and leave
// the specifier alone. Nothing has to be synthesised.
import { unwrap } from "solid-js";
// Several bindings and an existing declaration for the right module: take the
// specifier out of this list and append it to that one.
import { createSignal, createMemo } from "solid-js/store";
// Several bindings, and a declaration for the right module already present:
// append to that one rather than adding a second.
import { onMount, render } from "solid-js";

// Not a Solid module, so not this rule's business however familiar the name.
// `package-contract-incomplete` covers packages the checker has no model for.
import { createStore as fromElsewhere } from "@my/ui";

// A Solid subpath, and a name the bundled index has never heard of -- a patch
// release newer than the index, or somebody's re-export.
//
// SC7005 says nothing, and that is the case pinning the removed fallback:
// answering "otherwise, solid-js" makes this `newInPatch is not exported by
// solid-js/web; 1.x exports it from solid-js`, which is a fabricated module
// for a name the checker cannot see. The fallback was dangerous rather than
// merely wrong -- it turned every gap in the checker's knowledge into a
// confident finding.
//
// SC9001 does report it, and should: "this name is not in solid-checker's
// model of your solid-js" is a claim about the model, and its message says so.
// The two rules divide cleanly -- SC7005 places names it knows, SC9001 flags
// names it does not.
import { newInPatch } from "solid-js/web";

// A namespace import binds the module, not a name, so there is no name to
// place. A default import of a Solid module is meaningless already.
import * as web from "solid-js/web";

export function App() {
  const [count] = createSignal(0);
  const [state] = createStore({ a: 1 });
  const [other] = fromElsewhere({ b: 2 });
  const snapshot: Store<{ a: number }> = unwrap(state);
  const read: Accessor<number> = count;
  createMemo(() => count());
  onMount(() => {});
  render(() => null, null);
  return Portal({}) && Show({}) && snapshot && other && read && newInPatch() && web;
}
