import { createSignal } from "solid-js";
import * as namespace from "./wrappers";
import {
  TrackedProp, TrackedJsx, ForwardedProp, EagerProp, StoredProp, ReturnedProps,
  tracked, trackedTwice, effectCompute, computedSignal, explicitUntrack, mixedSafe,
  queued, inline, trackedAndInline, stored, returned, returnedUntrack, unknown,
  synchronousEvent, recursive, asyncConsumer, defaultConsumer, objectSlot,
  siblingInvoke, siblingValueOf, siblingTag, labelledUntrack, forwardGated, SiblingArrowProp,
  siblingOverwritten, prototypeWritten, ProtoConsumer, labelledInsideMemo, componentInsideMemo,
  forwardWithGate, siblingRead, unnamedRows, namedRows, caught, ReceiverProp, replay,
} from "./wrappers";
import { untrack } from "solid-js";
import { forwardTracked, forwardMixed, forwardInline } from "./forwarding";

// Proposed clean SC1001 results; no assertion about sibling rules.
export function DirectCompute() {
  const [count] = createSignal(0);
  const value = tracked(() => count());
  return <div>{value()}</div>;
}
export function MultipleComputes() {
  const [count] = createSignal(0);
  const values = trackedTwice(() => count());
  return <div>{values[0]() + values[1]()}</div>;
}
export function EffectCompute() {
  const [count] = createSignal(0);
  effectCompute(() => count());
  return <div />;
}
export function ComputedSignal() {
  const [count] = createSignal(0);
  const [value] = computedSignal(() => count());
  return <div>{value()}</div>;
}
export function ExplicitUntrack() {
  const [count] = createSignal(0);
  explicitUntrack(() => count());
  return <div />;
}
export function MixedSafe() {
  const [count] = createSignal(0);
  const value = mixedSafe(() => count());
  return <div>{value()}</div>;
}
export function FreshQueue() {
  const [count] = createSignal(0);
  queued(() => count());
  return <div />;
}
export function CrossFileTracked() {
  const [count] = createSignal(0);
  const value = forwardTracked((() => count()) satisfies (() => number));
  return <div>{value()}</div>;
}
export function CrossFileMixed() {
  const [count] = createSignal(0);
  const value = forwardMixed(() => count());
  return <div>{value()}</div>;
}
export function MemoProp() {
  const [count] = createSignal(0);
  return <TrackedProp read={() => count()} />;
}
export function JsxProp() {
  const [count] = createSignal(0);
  return <TrackedJsx read={() => count()} />;
}
export function ForwardedMemoProp() {
  const [count] = createSignal(0);
  return <ForwardedProp read={() => count()} />;
}
export function IndependentObjectSlots() {
  const [count] = createSignal(0);
  const value = objectSlot({ key: () => count(), load: () => count() });
  return <div>{value()}</div>;
}

// Violation cases: forwarding inline must never become clean.
export function InlineBody() {
  const [count] = createSignal(0);
  const value = inline(() => count());
  return <div>{value}</div>;
}
export function ForwardedInlineBody() {
  const [count] = createSignal(0);
  const value = forwardInline(() => count());
  return <div>{value}</div>;
}
export function SafeAndStrict() {
  const [count] = createSignal(0);
  const value = trackedAndInline(() => count());
  return <div>{value()}</div>;
}
export function EagerRenderProp() {
  const [count] = createSignal(0);
  return <EagerProp read={() => count()} />;
}

// Open uses: no new clean certificate is permitted.
export function StoredCallback() {
  const [count] = createSignal(0);
  stored(() => count());
  return <div />;
}
export function ReturnedCallback() {
  const [count] = createSignal(0);
  const later = returned(() => count());
  return <button onClick={() => later()}>read</button>;
}
export function ReturnedUntrackCapture() {
  const [count] = createSignal(0);
  const later = returnedUntrack(() => count());
  return <button onClick={() => later()}>read</button>;
}
export function UnknownPackage() {
  const [count] = createSignal(0);
  unknown(() => count());
  return <div />;
}
export function SynchronousDomDispatch() {
  const [count] = createSignal(0);
  synchronousEvent(() => count());
  return <div />;
}
export function StoredCallbackProp() {
  const [count] = createSignal(0);
  return <StoredProp read={() => count()} />;
}
export function WholePropsEscape() {
  const [count] = createSignal(0);
  return <ReturnedProps read={() => count()} />;
}
export function NamespaceTarget() {
  const [count] = createSignal(0);
  namespace.tracked(() => count());
  return <div />;
}
export function ComputedTarget(props: { consume: (read: () => number) => void }) {
  const [count] = createSignal(0);
  props["consume"](() => count());
  return <div />;
}
export function Cycle() {
  const [count] = createSignal(0);
  recursive(() => count(), 2);
  return <div />;
}
export function AsyncConsumer() {
  const [count] = createSignal(0);
  void asyncConsumer(() => count());
  return <div />;
}
export function DefaultedConsumer() {
  const [count] = createSignal(0);
  const value = defaultConsumer(() => count());
  return <div>{value()}</div>;
}
export function SpreadOverride() {
  const [count] = createSignal(0);
  const other = { read: () => 1 };
  return <TrackedProp {...other} read={() => count()} />;
}
export function SiblingInvoke() {
  const [count] = createSignal(0);
  siblingInvoke({ key: () => count(), invoke() { return this.key(); } });
  return <div />;
}
export function SiblingValueOf() {
  const [count] = createSignal(0);
  siblingValueOf({ key: () => count() });
  return <div />;
}
export function SiblingTag() {
  const [count] = createSignal(0);
  siblingTag({ key: () => count(), tag() { return this.key(); } });
  return <div />;
}
export function LabelledUntrack() {
  const [count] = createSignal(0);
  labelledUntrack(() => count());
  return <div />;
}
export function DirectLabelledUntrack() {
  const [count] = createSignal(0);
  untrack(() => count(), "direct label");
  return <div />;
}
export function GatedInline() {
  const [count] = createSignal(0);
  try {
    forwardGated(() => count());
  } catch {
    // The default prevents entering gatedInline's body.
  }
  return <div />;
}
export function SiblingArrowInvoke() {
  const [count] = createSignal(0);
  siblingInvoke({ key: () => count(), invoke: () => 1 });
  return <SiblingArrowProp read={() => count()} check={(n) => n > 0} />;
}
function checker(this: unknown, n: number) {
  return n > 0;
}
export function SiblingNamedInvoke() {
  const [count] = createSignal(0);
  return <SiblingArrowProp read={() => count()} check={checker} />;
}
const positive = (n: number) => n > 0;
export function SiblingConstArrow() {
  const [count] = createSignal(0);
  return <SiblingArrowProp read={() => count()} check={positive} />;
}
export function SiblingAbsent() {
  const [count] = createSignal(0);
  return <SiblingArrowProp read={() => count()} />;
}
export function SiblingOverwritten() {
  const [count] = createSignal(0);
  siblingOverwritten({ key: () => count(), invoke: () => 0 });
  return <div />;
}
export function PrototypeWritten() {
  const [count] = createSignal(0);
  prototypeWritten({ key: () => count() });
  return <div />;
}
const invokingPrototype = {
  invoke(this: { read: () => number }) { return this.read(); },
};
export function JsxPrototype() {
  const [count] = createSignal(0);
  return <ProtoConsumer read={() => count()} __proto__={invokingPrototype} />;
}
export function LabelledInsideMemo() {
  const [count] = createSignal(0);
  const value = labelledInsideMemo(() => count());
  return <div>{value()}</div>;
}
export function ComponentInsideMemo() {
  const [count] = createSignal(0);
  const value = componentInsideMemo(() => count());
  return <div>{String(value())}</div>;
}
export function GateArgument() {
  const [count] = createSignal(0);
  forwardWithGate(() => count());
  return <div />;
}
export function SiblingAbsentRead() {
  const [count] = createSignal(0);
  siblingRead({ read: () => count() });
  return <div />;
}
export function UnnamedRows() {
  const [count] = createSignal(0);
  const rows = unnamedRows(() => count());
  return <div>{rows().length}</div>;
}
export function NamedRows() {
  const [count] = createSignal(0);
  const rows = namedRows(() => count());
  return <div>{rows().length}</div>;
}
export function Caught() {
  const [count] = createSignal(0);
  caught(() => count());
  return <div />;
}
export function FunctionSubject() {
  const [count] = createSignal(0);
  return (
    <ReceiverProp
      read={function (this: { read: () => number }) {
        void this;
        return count();
      }}
      probe={replay()}
    />
  );
}
