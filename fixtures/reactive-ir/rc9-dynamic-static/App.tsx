// `dynamic(source, options?)` on @solidjs/web@2.0.0-rc.9. The runtime opens
// with `if (options?.static) return staticDynamic(untrack(source))`
// (`dist/web.dev.js:2199`): a literal `static: true` calls the source once,
// untracked, under the caller's owner, before `dynamic` returns. Every other
// proven option keeps the default lazy tracked memo, and an unproven one is
// modelled as neither.
import { createEffect, createSignal, untrack } from "solid-js";
import * as Web from "@solidjs/web";
import { dynamic, type DynamicOptions } from "@solidjs/web";

const [kind, setKind] = createSignal("a");
const Plain = () => <div />;

// --- The static form inherits the caller's owner ----------------------------

// Positive: at module scope there is no owner, so an effect created in a
// static source is unowned -- exactly as under `untrack` (below). The default
// form would have created it under its memo.
export const StaticEffect = dynamic(() => {
  createEffect(() => kind(), () => {});
  return Plain;
}, { static: true });

// The same claim through a namespace import: the form is chosen after the
// callee resolves, so the spelling of the import does not matter.
export const NamespaceStaticEffect = Web.dynamic(() => {
  createEffect(() => kind(), () => {});
  return Plain;
}, { static: true });

// Reference: what `staticDynamic(untrack(source))` does with the same body.
export const UntrackEffect = untrack(() => {
  createEffect(() => kind(), () => {});
  return Plain;
});

// --- The static form is not a tracked compute -------------------------------

// Negative: a write in a static source at module scope runs with no owner and
// no observer, which the write guard allows. The default form runs the same
// write inside its memo, where the guard throws (next case).
export const StaticWrite = dynamic(() => {
  setKind("b");
  return Plain;
}, { static: true });

// Control: the default form, unchanged from rc.3 -- the source is a tracked
// compute under the memo `dynamic` creates.
export const DefaultWrite = dynamic(() => {
  setKind("b");
  return Plain;
});

// Control: `static: false` and an exact literal without `static` are the
// default form at runtime (`options?.static` is falsy).
export const FalseWrite = dynamic(() => {
  setKind("b");
  return Plain;
}, { static: false });

export const DeferStreamWrite = dynamic(() => {
  setKind("b");
  return Plain;
}, { deferStream: true });

// Positive: `untrack` keeps the caller's owner, so inside a component body the
// write guard still throws for a static source -- the same answer as a write
// inside `untrack` there.
export function BodyStaticWrite() {
  const Resolved = dynamic(() => {
    setKind("c");
    return Plain;
  }, { static: true });
  return <Resolved />;
}

// Negative: a static source reads once, untracked, with no strict-read label
// (`untrack(source)`), so a prop read there is the deliberate one-shot read
// the option declares, not a missed subscription.
export function BodyStaticRead(props: { kind: string }) {
  const Resolved = dynamic(() => (props.kind === "a" ? Plain : "span"), { static: true });
  return <Resolved />;
}

// --- An unproven option states nothing --------------------------------------

// Negative: the options value is not proven, so the runtime may take either
// form. The dialect models neither: no owner, execution or write claim.
export function unknownOptions(options: DynamicOptions) {
  return dynamic(() => {
    createEffect(() => kind(), () => {});
    setKind("d");
    return Plain;
  }, options);
}

export function unknownFlag(flag: boolean) {
  return dynamic(() => {
    createEffect(() => kind(), () => {});
    setKind("d");
    return Plain;
  }, { static: flag });
}
