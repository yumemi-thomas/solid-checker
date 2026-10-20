import { children, createSignal, onCleanup, type Element } from "solid-js";

// ADR 0323: `children(fn)` is `createMemo(fn, { lazy: true })` in every
// audited release (`solid-js@2.0.0-rc.13` `dist/solid.js:21-24`). `fn` is a
// computation of its own: a read in it subscribes that memo, so it is neither
// a violation nor an obligation, for signals, props and stores alike.

// Signal reads in the callback: tracked, clean.
export function SignalInChildren(props: { children: Element }) {
  const [n] = createSignal(1);
  const resolved = children(() => (n() > 0 ? props.children : null));
  const second = children(() => {
    n();
    return props.children;
  });
  return <div>{resolved()}{second()}</div>;
}

// Props reads: the idiom, tracked, clean.
export function PropsInChildren(props: { children: Element }) {
  const resolved = children(() => props.children);
  return <div>{resolved.toArray()}</div>;
}

// The same reads under a deferred primitive's callback are also clean (control).
export function CleanupControl() {
  const [n] = createSignal(1);
  onCleanup(() => {
    n();
  });
  return <div />;
}

// Positive control: a read written directly in the body is still a proven
// violation, so the row did not silence the rule.
export function DirectRead() {
  const [n] = createSignal(1);
  const snapshot = n();
  return <div>{snapshot}</div>;
}

// Control against matching by name: a project helper that merely resembles
// `children` is not the primitive. It calls its parameter in its own body, so
// the read is a proven violation exactly as before the row existed.
function children2(fn: () => unknown) {
  return fn();
}
export function HomonymControl() {
  const [n] = createSignal(1);
  children2(() => n());
  return <div />;
}
