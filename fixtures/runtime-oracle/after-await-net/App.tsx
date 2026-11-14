import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B follow-up (ADR 0976), S1 and R6 of the reviews of #233: code the async function runs after its `await` with no call written there, in a program where no
// computation is recognized (each function is a declaration handed by identifier to a project wrapper that gives its parameter to `createMemo`: a start the producers' index does not
// follow). A getter, a destructuring and a spread of an object with a getter, a coercion (`toString`), a `for…of` over an iterable, a `new` whose constructor reads, a tagged template,
// a Proxy's `get` and `has` trap, `toJSON`, and a stored generator object spread, iterated or destructured. Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read inside the
// member the continuation runs without a call; the member-route net (ADR 0963) rows that read once its gate is "any suspension in the program", and a generator body is one of its
// members. Each definition is written on one line (the judge anchors a finding at its start line).
const holder = { get value() { return source(); } };
const coerced = { toString() { return String(source()); } };
const iterable = { *[Symbol.iterator]() { yield source(); } };
class Reads { v: number; constructor() { this.v = source(); } }
function tag(parts: TemplateStringsArray) { return parts.length + source(); }
const proxy: any = new Proxy({}, { get: () => source() });
const probed: any = new Proxy({}, { has: () => source() > 0 });
function* numbers() { yield source(); }
const stored = numbers();
const storedLoop = numbers();
const storedPattern = numbers();
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
async function viaGetter() {
  await pause();
  return holder.value;
}
async function viaDestructure() {
  await pause();
  const { value } = holder;
  return value;
}
async function viaSpread() {
  await pause();
  return { ...holder }.value;
}
async function viaCoerce() {
  await pause();
  return `${coerced}`;
}
async function viaForOf() {
  await pause();
  let total = 0;
  for (const value of iterable) total += value;
  return total;
}
async function viaNew() {
  await pause();
  return new Reads().v;
}
async function viaTag() {
  await pause();
  return tag`x`;
}
async function viaProxy() {
  await pause();
  return proxy.x;
}
async function viaIn() {
  await pause();
  return "x" in probed;
}
async function viaStoredSpread() {
  await pause();
  return [...stored][0];
}
async function viaStoredLoop() {
  await pause();
  for (const value of storedLoop) return value;
  return 0;
}
async function viaStoredPattern() {
  await pause();
  const [value] = storedPattern;
  return value;
}

createRoot(() => {
  track(viaGetter);
  track(viaDestructure);
  track(viaSpread);
  track(viaCoerce);
  track(viaForOf);
  track(viaNew);
  track(viaTag);
  track(viaProxy);
  track(viaIn);
  track(viaStoredSpread);
  track(viaStoredLoop);
  track(viaStoredPattern);
});

export function Probe() {
  return <p>probe</p>;
}
