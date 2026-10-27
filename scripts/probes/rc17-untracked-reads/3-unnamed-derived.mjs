// 3-unnamed-derived.mjs
// Q3: the badUnnamedDerivedSignal shapes. For each case: dev diagnostics, subscribers on
// the signal, and whether a write reaches the consumer. Upstream console.log(...) calls are
// replaced by logged.push(...) so the output is clean; the read itself is unchanged.
// 3a/3b user examples: createEffect(() => () => count(), fn), createMemo(() => () => count())
// 3c function returned from a component body, read in a tracked effect, a parent body, top level
// 3d upstream #32 (runWithOwner in createEffect, root owner), plus the one-argument form
// 3e upstream #33 (same, inside a component)
// 3f upstream #34 (props conditional; the JSX style attribute is emulated with createRenderEffect)
// 3g upstream #35 (dynamic(() => test(count()))); some-lib is not installed, so the control
//    is Solid's own dynamic from @solidjs/web, with a component-returning source
import { createComponent, createEffect, createMemo, createRenderEffect, createRoot, createSignal, flush, getOwner, runWithOwner } from "solid-js";
import { dynamic } from "@solidjs/web";
import { mark, observers, report, resolved } from "./lib.mjs";

const show = (label, value) => console.log(`  ${label}: ${JSON.stringify(value)}`);
console.log("resolved", JSON.stringify(resolved));

console.log("== 3a. createEffect(() => () => count(), effectFn)");
{
  const [count, setCount] = createSignal(1, { name: "count" });
  let effectRuns = 0;
  let received;
  let m = mark();
  createRoot(() => {
    createEffect(() => () => count(), (fn) => { effectRuns++; received = fn; });
  });
  flush();
  show("effectRuns after creation", effectRuns);
  show("effectFn received", typeof received);
  show("observers(count)", observers(count));
  report("3a create", m);
  m = mark();
  setCount(2);
  flush();
  show("effectRuns after setCount(2)+flush", effectRuns);
  show("received() top-level call", received());
  show("observers(count)", observers(count));
  report("3a write", m);
}

console.log("== 3a-2. same as 3a, but the effect callback calls the function it received");
{
  const [count, setCount] = createSignal(1, { name: "count2" });
  let effRuns = 0;
  let seen;
  let m = mark();
  createRoot(() => {
    createEffect(() => () => count(), (fn) => { effRuns++; seen = fn(); });
  });
  flush();
  show("effRuns after creation", effRuns);
  show("seen (fn() called inside the effect callback)", seen);
  report("3a-2 create", m);
  m = mark();
  setCount(2);
  flush();
  show("effRuns after setCount(2)+flush", effRuns);
  show("seen after setCount(2)", seen);
  show("observers(count)", observers(count));
  report("3a-2 write", m);
}

console.log("== 3b. createMemo(() => () => count()), then a tracked consumer calls memo()()");
{
  const [count, setCount] = createSignal(1, { name: "count" });
  let memoRuns = 0;
  let consumerRuns = 0;
  let consumerSeen;
  let m = mark();
  const memo = createMemo(() => { memoRuns++; return () => count(); }, { name: "fnMemo" });
  show("memoRuns after creation", memoRuns);
  show("typeof memo()", typeof memo());
  show("memo()() top-level", memo()());
  show("observers(count) before consumer", observers(count));
  report("3b create", m);
  m = mark();
  createRoot(() => {
    createRenderEffect(() => { consumerRuns++; consumerSeen = memo()(); }, () => {});
  });
  flush();
  show("consumerRuns", consumerRuns);
  show("consumerSeen", consumerSeen);
  show("observers(count) after consumer", observers(count));
  report("3b consumer", m);
  m = mark();
  setCount(2);
  flush();
  show("memoRuns after setCount(2)", memoRuns);
  show("consumerRuns after setCount(2)", consumerRuns);
  show("consumerSeen after setCount(2)", consumerSeen);
  show("memo()() top-level after write", memo()());
  report("3b write", m);
}

console.log("== 3c. component body returns () => v(); read in a tracked effect, a parent body, top level");
{
  let vAcc;
  let setV;
  function Getter() {
    const [v, set] = createSignal(1, { name: "v" });
    vAcc = v;
    setV = set;
    return () => v();
  }
  const get = createRoot(() => createComponent(Getter, {}));
  let trackedRuns = 0;
  let tracked;
  let m = mark();
  createRoot(() => {
    createRenderEffect(() => { trackedRuns++; tracked = get(); }, () => {});
  });
  flush();
  show("(i) tracked effect: runs after creation", trackedRuns);
  show("(i) tracked effect: value", tracked);
  report("3c (i) create", m);
  m = mark();
  let parentSeen;
  function Parent() {
    parentSeen = get();
    return null;
  }
  createRoot(() => createComponent(Parent, {}));
  flush();
  show("(ii) parent body read at creation", parentSeen);
  report("3c (ii) parent body", m);
  m = mark();
  setV(2);
  flush();
  show("(i) tracked effect: runs after setV(2)", trackedRuns);
  show("(i) tracked effect: value after setV(2)", tracked);
  show("(ii) parent body value after setV(2)", parentSeen);
  show("(iii) get() at top level after setV(2)", get());
  show("observers(v)", observers(vAcc));
  report("3c write", m);
}

console.log("== 3d. upstream #32: createEffect(() => runWithOwner(owner, () => log(signal()))), root owner");
{
  const [signal, setSignal] = createSignal(0, { name: "signal" });
  const logged = [];
  let runs = 0;
  let owner;
  let m = mark();
  createRoot(() => {
    owner = getOwner();
    createEffect(() => { runs++; return runWithOwner(owner, () => logged.push(signal())); }, () => {});
  });
  flush();
  show("runs after creation", runs);
  show("logged after creation", logged);
  show("observers(signal)", observers(signal));
  report("3d create", m);
  m = mark();
  setSignal(1);
  flush();
  show("runs after setSignal(1)", runs);
  show("logged after setSignal(1)", logged);
  show("observers(signal)", observers(signal));
  report("3d write", m);
  m = mark();
  let err;
  try {
    createRoot(() => {
      const o = getOwner();
      createEffect(() => runWithOwner(o, () => logged.push(signal())));
    });
  } catch (e) {
    err = String(e.message);
  }
  show("upstream one-argument createEffect threw", err);
  report("3d one-argument form", m);
}

console.log("== 3e. upstream #33: the same shape inside a component");
{
  const logged = [];
  let runs = 0;
  let setS;
  let sAcc;
  function Component() {
    const owner = getOwner();
    const [signal, set] = createSignal(0, { name: "signal" });
    sAcc = signal;
    setS = set;
    createEffect(() => { runs++; return runWithOwner(owner, () => logged.push(signal())); }, () => {});
    return null;
  }
  let m = mark();
  createRoot(() => createComponent(Component, {}));
  flush();
  show("runs after creation", runs);
  show("logged after creation", logged);
  show("observers(signal)", observers(sAcc));
  report("3e create", m);
  m = mark();
  setS(1);
  flush();
  show("runs after setS(1)", runs);
  show("logged after setS(1)", logged);
  report("3e write", m);
}

console.log("== 3f. upstream #34: props.big ? () => ({ fontSize: props.size }) : undefined; style && style() in JSX");
{
  const [big, setBig] = createSignal(true, { name: "big" });
  const [size, setSize] = createSignal(10, { name: "size" });
  let styleOut;
  let styleRuns = 0;
  function Card(props) {
    const style = props.big ? () => ({ fontSize: props.size }) : undefined;
    createRenderEffect(() => { styleRuns++; styleOut = style && style(); }, () => {});
    return null;
  }
  let m = mark();
  createRoot(() => createComponent(Card, { get big() { return big(); }, get size() { return size(); } }));
  flush();
  show("big=true, size=10: style output", styleOut);
  report("3f create (big=true)", m);
  m = mark();
  setSize(12);
  flush();
  show("after setSize(12): style output", styleOut);
  report("3f size write", m);
  m = mark();
  setBig(false);
  flush();
  show("after setBig(false): style output (condition read once in the body)", styleOut);
  show("styleRuns", styleRuns);
  report("3f big write", m);
  m = mark();
  const [big2, setBig2] = createSignal(false, { name: "big2" });
  let styleOut2;
  function Card2(props) {
    const style = props.big ? () => ({ fontSize: props.size }) : undefined;
    createRenderEffect(() => { styleOut2 = style && style(); }, () => {});
    return null;
  }
  createRoot(() => createComponent(Card2, { get big() { return big2(); }, get size() { return size(); } }));
  flush();
  show("second instance, big=false at setup: style output", styleOut2);
  setBig2(true);
  flush();
  show("second instance, after setBig2(true): style output", styleOut2);
  report("3f second instance", m);
}

console.log("== 3g. upstream #35: dynamic(() => test(count())); control = @solidjs/web dynamic, component-returning source");
{
  const [count, setCount] = createSignal(1, { name: "count" });
  const A = () => "A";
  const B = () => "B";
  let sourceRuns = 0;
  const pick = (n) => (n > 1 ? B : A);
  let m = mark();
  let Test;
  let out;
  try {
    Test = dynamic(() => { sourceRuns++; return pick(count()); });
    createRoot(() => { createRenderEffect(() => { out = Test({}); }, () => {}); });
    flush();
    show("sourceRuns after creation", sourceRuns);
    show("typeof Test({}) result (createMemo accessor, solid.dev.js:1504)", typeof out);
    show("rendered value = out()", out());
    show("observers(count)", observers(count));
    report("3g create", m);
    m = mark();
    setCount(2);
    flush();
    show("sourceRuns after setCount(2)", sourceRuns);
    show("rendered value after setCount(2) = out()", out());
  } catch (e) {
    show("control threw", String(e.message).slice(0, 160));
  }
  report("3g", m);
}
