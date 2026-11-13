import { createMemo } from "solid-js";
import { makeKernel } from "@ws/kernel";
import { makeCalm } from "calm";

// Both packages are workspace packages linked into node_modules: the compiler resolves them to their real directories (`packages/kernel`,
// `packages/calm`), so a member's declaration is in no node_modules. The objects are made once, at module scope, where no owner is active.
const kernel = makeKernel();
const calm = makeCalm();

// Uncertifiable: `@ws/kernel`'s closure loads solid-js, and its member runs where an owner is active.
export function KernelBody() {
  kernel.run(() => {});
  return <div />;
}
export function KernelMemo() {
  const value = createMemo(() => {
    kernel.run(() => {});
    return 1;
  });
  return <div>{value()}</div>;
}
// Uncertifiable: the import's own call, in a body (its declaration is named by the import's `node_modules` spelling).
export function KernelMake() {
  makeKernel();
  return <div />;
}

// Silent: a timer callback and an event handler are legal action sites.
export function KernelTimer() {
  setTimeout(() => kernel.run(() => {}), 0);
  return <div />;
}
export function KernelHandler() {
  return <button onClick={() => kernel.run(() => {})}>run</button>;
}
// Silent: `calm`'s closure names no Solid package, loads nothing by a computed name and reads only the libraries' globals.
export function CalmBody() {
  calm.run(() => {});
  return <div />;
}
