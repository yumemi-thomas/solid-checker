// `dynamic(source, { static: true })` on the @solidjs/web@2.0.0-rc.7 triple.
// rc.7 declares `dynamic(source, _options?: DynamicOptions)` with
// `DynamicOptions { deferStream?: boolean }`, so `{ static: true }` is TS2353,
// and its client bundle never reads `_options` (`dist/web.dev.js:2042`): the
// source is the lazy tracked memo's compute, under the owner that memo
// creates, whatever the call passes. So each static call must answer exactly
// as its one-argument twin -- rc.9's static form (no owner, untracked) is not
// what these bytes do.
import { createEffect, createSignal, untrack } from "solid-js";
import { dynamic } from "@solidjs/web";

const [kind, setKind] = createSignal("a");
const Plain = () => <div />;

// An effect in the source is created under the memo `dynamic` creates, on both
// calls. rc.9's model would call the first one unowned (SC4001).
export const StaticEffect = dynamic(() => {
  createEffect(() => kind(), () => {});
  return Plain;
}, { static: true });

export const DefaultEffect = dynamic(() => {
  createEffect(() => kind(), () => {});
  return Plain;
});

// A write in the source runs inside the memo's tracked compute on both calls,
// where the write guard throws. rc.9's model would call the first one legal.
export const StaticWrite = dynamic(() => {
  setKind("b");
  return Plain;
}, { static: true });

export const DefaultWrite = dynamic(() => {
  setKind("b");
  return Plain;
});

// tsc-clean on rc.7, and the same default form.
export const DeferStreamWrite = dynamic(() => {
  setKind("b");
  return Plain;
}, { deferStream: true });

// Reference: what rc.9's static form would be, and rc.7's is not.
export const UntrackEffect = untrack(() => {
  createEffect(() => kind(), () => {});
  return Plain;
});
