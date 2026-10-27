// 2-context-provider.mjs
// Q2: <Ctx value={count()}> compiles to
//   createComponent(Ctx, { get value() { return count(); }, get children() { ... } })
// Does a child's tracked memo that calls useContext(Ctx) see a later write to count?
// Control: a second memo in the same child reads count() directly, so the harness
// is shown to re-run tracked memos after a write.
import { createComponent, createContext, createMemo, createRenderEffect, createRoot, createSignal, flush, getOwner, runWithOwner, useContext } from "solid-js";
import { mark, observers, report, resolved } from "./lib.mjs";

const show = (label, value) => console.log(`  ${label}: ${JSON.stringify(value)}`);
console.log("resolved", JSON.stringify(resolved));

const Ctx = createContext("default", { name: "Ctx" });
const [count, setCount] = createSignal(1, { name: "count" });
let valueReads = 0;
let childRuns = 0;
let controlRuns = 0;
let childMemo;
let controlMemo;
let childOwner;

function Child() {
  childOwner = getOwner();
  childMemo = createMemo(() => { childRuns++; return useContext(Ctx); }, { name: "childMemo" });
  controlMemo = createMemo(() => { controlRuns++; return count(); }, { name: "controlMemo" });
  return null;
}

console.log("== provider creation: createComponent(Ctx, { get value() { return count(); }, get children() {...} })");
let m = mark();
let tree;
createRoot(() => {
  tree = createComponent(Ctx, {
    get value() { valueReads++; return count(); },
    get children() { return createComponent(Child, {}); },
  });
});
show("valueReads after creation", valueReads);
report("provider creation", m);

console.log("== mount the children (the provider's children memo is read inside a render effect)");
m = mark();
createRoot(() => { createRenderEffect(() => { tree(); }, () => {}); });
flush();
show("childRuns", childRuns);
show("controlRuns", controlRuns);
show("childMemo value", childMemo());
show("controlMemo value", controlMemo());
show("valueReads", valueReads);
show("observers(count)", observers(count));
report("mount", m);

console.log("== setCount(2) + flush");
m = mark();
setCount(2);
flush();
show("childRuns", childRuns);
show("controlRuns", controlRuns);
show("childMemo value (top-level read)", childMemo());
show("controlMemo value (top-level read)", controlMemo());
show("useContext(Ctx) run inside the child's owner now", runWithOwner(childOwner, () => useContext(Ctx)));
show("valueReads", valueReads);
show("observers(count)", observers(count));
report("setCount(2)", m);
