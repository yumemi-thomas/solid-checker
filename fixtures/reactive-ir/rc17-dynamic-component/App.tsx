// `dynamicComponent(source, options?)` on solid-js@2.0.0-rc.17 is
// `dynamicCore` with no tag arm (`dist/solid.dev.js:1545`), the core web's
// `dynamic` forwards to: `if (options?.static) return
// staticDynamic(untrack(source), tagArm)` (`:1445`). A literal `static: true`
// calls the source once, untracked, under the caller's owner; every other
// proven option keeps the lazy tracked memo, and an unproven one is modelled
// as neither. The same claims as `rc9-dynamic-static`, through each spelling.
import { createEffect, createSignal, untrack, dynamicComponent, type DynamicOptions } from "solid-js";
import * as Solid from "solid-js";
import { dynamicComponent as webDynamicComponent } from "@solidjs/web";

const [kind, setKind] = createSignal("a");
const Plain = () => <div />;
const Other = () => <span />;

// --- The static form inherits the caller's owner ----------------------------

// Positive: at module scope there is no owner, so an effect created in a
// static source is unowned (SC4001), exactly as under `untrack` (below).
export const StaticEffect = dynamicComponent(() => {
  createEffect(() => kind(), () => {});
  return Plain;
}, { static: true });

// The same claim through a namespace import and through `@solidjs/web`'s
// re-export, which is the same function (`dist/web.dev.js:2`).
export const NamespaceStaticEffect = Solid.dynamicComponent(() => {
  createEffect(() => kind(), () => {});
  return Plain;
}, { static: true });

export const WebStaticEffect = webDynamicComponent(() => {
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
// no observer, which the write guard allows.
export const StaticWrite = dynamicComponent(() => {
  setKind("b");
  return Plain;
}, { static: true });

// Positive: the default form's source is a tracked compute under the memo
// `dynamicCore` creates, where the guard throws (SC2001).
export const DefaultWrite = dynamicComponent(() => {
  setKind("b");
  return Plain;
});

// Positive: `static: false` and an exact literal without `static` are the
// default form at runtime (`options?.static` is falsy).
export const FalseWrite = dynamicComponent(() => {
  setKind("b");
  return Plain;
}, { static: false });

export const DeferStreamWrite = webDynamicComponent(() => {
  setKind("b");
  return Plain;
}, { deferStream: true });

// Positive: `untrack` keeps the caller's owner, so inside a component body the
// write guard still throws for a static source.
export function BodyStaticWrite() {
  const Resolved = dynamicComponent(() => {
    setKind("c");
    return Plain;
  }, { static: true });
  return <Resolved />;
}

// Negative: a static source reads once, untracked, with no strict-read label.
export function BodyStaticRead(props: { kind: string }) {
  const Resolved = dynamicComponent(() => (props.kind === "a" ? Plain : Other), { static: true });
  return <Resolved />;
}

// --- An unproven option states nothing --------------------------------------

// Uncertifiable: the options value is not proven, so the runtime may take
// either form. The dialect models neither.
export function unknownOptions(options: DynamicOptions) {
  return dynamicComponent(() => {
    createEffect(() => kind(), () => {});
    setKind("d");
    return Plain;
  }, options);
}

export function unknownFlag(flag: boolean) {
  return dynamicComponent(() => {
    createEffect(() => kind(), () => {});
    setKind("d");
    return Plain;
  }, { static: flag });
}

// --- SC2007: the static form's source must be synchronous -------------------

// Positive: `staticDynamic` throws on a thenable in dev (`dist/solid.dev.js:1530`)
// and renders nothing in prod (`dist/solid.js`, `return () => undefined;`).
export const StaticAsync = dynamicComponent(async () => Plain, { static: true });

// Negative: the default form settles the source in its memo.
export const DefaultAsync = dynamicComponent(async () => Plain);
