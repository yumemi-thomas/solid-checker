// 1-mapper-window.mjs
// Q1: a signal read inside a mapArray or repeat mapper. Does it raise
// STRICT_READ_UNTRACKED, get tracked by the mapper's owner or the mapArray node,
// or neither? Parts: A/B module scope, C module scope with { name }, D/F component
// body (createComponent), E/G component body with { name }, H control (read in the
// list function, which is tracked).
import { createComponent, createRenderEffect, createRoot, createSignal, flush, mapArray, repeat } from "solid-js";
import { mark, observers, report, resolved } from "./lib.mjs";

const show = (label, value) => console.log(`  ${label}: ${JSON.stringify(value)}`);
console.log("resolved", JSON.stringify(resolved));

console.log("== A. module scope (no root, no component), mapArray, no name option");
{
  const [list] = createSignal([1, 2], { name: "aList" });
  const [tag, setTag] = createSignal("a", { name: "aTag" });
  let runs = 0;
  let m = mark();
  const mapped = mapArray(list, (item) => { runs++; return `${item}${tag()}`; });
  show("A1 value (top-level read)", mapped());
  show("A1 mapper runs", runs);
  show("A1 observers(tag)", observers(tag));
  report("A1", m);
  m = mark();
  setTag("b");
  flush();
  show("A2 after setTag('b')+flush: value", mapped());
  show("A2 mapper runs", runs);
  show("A2 observers(tag)", observers(tag));
  report("A2", m);
}

console.log("== B. module scope, repeat, no name option");
{
  const [count] = createSignal(2, { name: "bCount" });
  const [tag, setTag] = createSignal("a", { name: "bTag" });
  let runs = 0;
  let m = mark();
  const rep = repeat(count, (i) => { runs++; return `${i}${tag()}`; });
  show("B1 value", rep());
  show("B1 mapper runs", runs);
  show("B1 observers(tag)", observers(tag));
  report("B1", m);
  m = mark();
  setTag("b");
  flush();
  show("B2 after setTag('b')+flush: value", rep());
  show("B2 mapper runs", runs);
  report("B2", m);
}

console.log('== B2. module scope, repeat, { name: "bRep" }');
{
  const [count] = createSignal(2, { name: "b2Count" });
  const [tag, setTag] = createSignal("a", { name: "b2Tag" });
  let runs = 0;
  let m = mark();
  const rep = repeat(count, (i) => { runs++; return `${i}${tag()}`; }, { name: "bRep" });
  show("B2 value", rep());
  show("B2 mapper runs", runs);
  show("B2 observers(tag)", observers(tag));
  report("B2 create", m);
  m = mark();
  setTag("b");
  flush();
  show("B2 after setTag('b')+flush: value", rep());
  show("B2 mapper runs", runs);
  report("B2 write", m);
}

console.log('== C. module scope, mapArray, { name: "cRows" }');
{
  const [list] = createSignal([1, 2], { name: "cList" });
  const [tag, setTag] = createSignal("a", { name: "cTag" });
  let runs = 0;
  let m = mark();
  const mapped = mapArray(list, (item) => { runs++; return `${item}${tag()}`; }, { name: "cRows" });
  show("C1 value", mapped());
  show("C1 mapper runs", runs);
  show("C1 observers(tag)", observers(tag));
  report("C1", m);
  m = mark();
  setTag("b");
  flush();
  show("C2 after setTag('b')+flush: value", mapped());
  show("C2 mapper runs", runs);
  report("C2", m);
}

// Component-body parts. The component returns handles so the test can write
// to the signal and read the output later.
let dRuns = 0;
function DRows() {
  const [list] = createSignal([1, 2], { name: "dList" });
  const [tag, setTag] = createSignal("a", { name: "dTag" });
  const out = mapArray(list, (item) => { dRuns++; return `${item}${tag()}`; });
  return { out, tag, setTag };
}
console.log("== D. component body (createComponent), mapArray, no name option");
{
  let m = mark();
  const h = createRoot(() => createComponent(DRows, {}));
  let seen;
  createRoot(() => { createRenderEffect(() => { seen = h.out(); }, () => {}); });
  flush();
  show("D1 tracked render-effect value", seen);
  show("D1 mapper runs", dRuns);
  show("D1 observers(tag)", observers(h.tag));
  report("D1", m);
  m = mark();
  h.setTag("b");
  flush();
  show("D2 tracked render-effect value after setTag('b')", seen);
  show("D2 top-level read value", h.out());
  show("D2 mapper runs", dRuns);
  report("D2", m);
}

let eRuns = 0;
function ERows() {
  const [list] = createSignal([1, 2], { name: "eList" });
  const [tag, setTag] = createSignal("a", { name: "eTag" });
  const out = mapArray(list, (item) => { eRuns++; return `${item}${tag()}`; }, { name: "eRows" });
  return { out, tag, setTag };
}
console.log('== E. component body, mapArray, { name: "eRows" }');
{
  let m = mark();
  const h = createRoot(() => createComponent(ERows, {}));
  let seen;
  createRoot(() => { createRenderEffect(() => { seen = h.out(); }, () => {}); });
  flush();
  show("E1 tracked render-effect value", seen);
  show("E1 mapper runs", eRuns);
  show("E1 observers(tag)", observers(h.tag));
  report("E1", m);
  m = mark();
  h.setTag("b");
  flush();
  show("E2 tracked render-effect value after setTag('b')", seen);
  show("E2 mapper runs", eRuns);
  report("E2", m);
}

let fRuns = 0;
function FRep() {
  const [count] = createSignal(2, { name: "fCount" });
  const [tag, setTag] = createSignal("a", { name: "fTag" });
  const out = repeat(count, (i) => { fRuns++; return `${i}${tag()}`; });
  return { out, tag, setTag };
}
console.log("== F. component body, repeat, no name option");
{
  let m = mark();
  const h = createRoot(() => createComponent(FRep, {}));
  let seen;
  createRoot(() => { createRenderEffect(() => { seen = h.out(); }, () => {}); });
  flush();
  show("F1 tracked render-effect value", seen);
  show("F1 mapper runs", fRuns);
  show("F1 observers(tag)", observers(h.tag));
  report("F1", m);
  m = mark();
  h.setTag("b");
  flush();
  show("F2 tracked render-effect value after setTag('b')", seen);
  show("F2 mapper runs", fRuns);
  report("F2", m);
}

let gRuns = 0;
function GRep() {
  const [count] = createSignal(2, { name: "gCount" });
  const [tag, setTag] = createSignal("a", { name: "gTag" });
  const out = repeat(count, (i) => { gRuns++; return `${i}${tag()}`; }, { name: "gRep" });
  return { out, tag, setTag };
}
console.log('== G. component body, repeat, { name: "gRep" }');
{
  let m = mark();
  const h = createRoot(() => createComponent(GRep, {}));
  let seen;
  createRoot(() => { createRenderEffect(() => { seen = h.out(); }, () => {}); });
  flush();
  show("G1 tracked render-effect value", seen);
  show("G1 mapper runs", gRuns);
  show("G1 observers(tag)", observers(h.tag));
  report("G1", m);
  m = mark();
  h.setTag("b");
  flush();
  show("G2 tracked render-effect value after setTag('b')", seen);
  show("G2 mapper runs", gRuns);
  report("G2", m);
}

console.log("== H. control: component body, mapArray, read inside the list function (tracked)");
{
  let hRuns = 0;
  function HRows() {
    const [tag, setTag] = createSignal("a", { name: "hTag" });
    const out = mapArray(() => [1, 2].map((x) => `${x}${tag()}`), (item) => { hRuns++; return item; });
    return { out, setTag, tag };
  }
  let m = mark();
  const h = createRoot(() => createComponent(HRows, {}));
  let seen;
  createRoot(() => { createRenderEffect(() => { seen = h.out(); }, () => {}); });
  flush();
  show("H1 tracked render-effect value", seen);
  show("H1 mapper runs", hRuns);
  show("H1 observers(tag)", observers(h.tag));
  report("H1", m);
  m = mark();
  h.setTag("b");
  flush();
  show("H2 tracked render-effect value after setTag('b')", seen);
  show("H2 mapper runs", hRuns);
  report("H2", m);
}
