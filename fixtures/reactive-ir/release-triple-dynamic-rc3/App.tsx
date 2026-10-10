// `dynamic(source, { static: true })` on the @solidjs/web@2.0.0-rc.3 triple.
// rc.3 declares `dynamic(source)` with one parameter, so every two-argument
// call below is TS2554, and its runtime never reads a second argument: the
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

// Reference: what rc.9's static form would be, and rc.3's is not.
export const UntrackEffect = untrack(() => {
  createEffect(() => kind(), () => {});
  return Plain;
});
