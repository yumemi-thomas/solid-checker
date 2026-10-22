import { createSignal, merge, omit } from "solid-js";
import * as Solid from "solid-js";
import { ShadowedOmit } from "./Shadowed";

type P = { value: string; class?: string; label?: string };

// ---- Accepted: the whole-object use cannot write, delete or alias the props.

// Violation: `omit` is always a view (`set`/`deleteProperty` are no-ops), so
// the setup read of the reactive `value` is still the caller's getter.
function OmitView(props: P) {
  const rest = omit(props, "class");
  const v = props.value;
  return <input {...rest} value={v} />;
}

// Violation: `omit` with no keys is still a view (no shortcut for zero keys).
function OmitNoKeys(props: P) {
  const rest = omit(props);
  const v = props.value;
  return <input {...rest} value={v} />;
}

// Violation: `merge` with an object literal beside the props is a view.
function MergeView(props: P) {
  const merged = merge(props, { label: "x" });
  const v = props.value;
  return <span title={merged.label}>{v}</span>;
}

// Violation: the props need not be the first source.
function MergeLaterSource(props: P) {
  const merged = merge({ label: "x" }, props);
  const v = props.value;
  return <span title={merged.label}>{v}</span>;
}

// Violation: a function source (a memo) is never falsy either.
function MergeFunctionSource(props: P) {
  const merged = merge(props, () => ({ label: "x" }));
  const v = props.value;
  return <span title={merged.label}>{v}</span>;
}

// Violation: `Solid.omit` is the same exact export.
function NamespaceOmit(props: P) {
  const rest = Solid.omit(props, "class");
  const v = props.value;
  return <input {...rest} value={v} />;
}

// Violation: a view of a view.
function NestedViews(props: P) {
  const rest = omit(merge(props, { label: "x" }), "class");
  const v = props.value;
  return <input {...rest} value={v} />;
}

// Violation: a `const` alias used only as a view source stays the same root.
function AliasThenOmit(props: P) {
  const local = props;
  const rest = omit(local, "class");
  const v = props.value;
  return <input {...rest} value={v} />;
}

// Violation: `@solidjs/web` `spread`/`ssrElement` only read the object they
// are given, so spreading props onto an intrinsic element is inert.
function IntrinsicSpread(props: P) {
  const v = props.value;
  return <input {...props} data-v={v} />;
}

// ---- Kept uncertifiable: each could write or alias the props object.

// `merge(props)` is the props object itself, so the write reaches it.
function MergeSingleSource(props: P) {
  const same = merge(props);
  same.value = "reset";
  const v = props.value;
  return <span>{v}</span>;
}

// `maybe` may be undefined, leaving the props as the only non-falsy source.
function MergeMaybeSource(props: P, maybe?: { label: string }) {
  const same = merge(props, maybe);
  same.value = "reset";
  const v = props.value;
  return <span>{v}</span>;
}

// A spread argument may expand to nothing.
function MergeSpreadSource(props: P, extra: { label: string }[]) {
  const same = merge(props, ...extra);
  same.value = "reset";
  const v = props.value;
  return <span>{v}</span>;
}

// A plain alias written through reaches the object.
function AliasWrite(props: P) {
  const local = props;
  local.value = "reset";
  const v = props.value;
  return <span>{v}</span>;
}

// An alias handed to a function escapes.
declare function consume(value: object): void;
function AliasEscape(props: P) {
  const local = props;
  consume(local);
  const v = props.value;
  return <span>{v}</span>;
}

// A destructured binding of the props is not a view.
function Destructured(props: P) {
  const { class: cls, ...others } = props;
  const v = props.value;
  return <input {...others} class={cls} value={v} />;
}

// `ShadowedOmit` (Shadowed.tsx) calls a local `omit` that is not Solid's.

// A write through the props before an intrinsic spread: the spread is inert,
// the write is not.
function IntrinsicSpreadWrite(props: P) {
  props.value = "reset";
  const v = props.value;
  return <input {...props} data-v={v} />;
}

// A write through the props themselves, view or not.
function OmitThenWrite(props: P) {
  const rest = omit(props, "class");
  props.value = "reset";
  const v = props.value;
  return <input {...rest} value={v} />;
}

// A spread onto a component is not an intrinsic spread.
function Sink(props: P) {
  return <span>{props.label}</span>;
}
function ComponentSpread(props: P) {
  const v = props.value;
  return <Sink {...props} label={v} />;
}

// Reading through the view is another question (key mapping): kept.
function ReadThroughView(props: P) {
  const rest = omit(props, "class");
  const v = rest.value;
  return <input value={v} />;
}

// Static callers: every call site passes a literal, nothing reactive to warn.
function StaticOmit(props: P) {
  const rest = omit(props, "class");
  const v = props.value;
  return <input {...rest} value={v} />;
}

export function App() {
  const [text] = createSignal("a");
  return (
    <div>
      <OmitView value={text()} />
      <OmitNoKeys value={text()} />
      <MergeView value={text()} />
      <MergeLaterSource value={text()} />
      <MergeFunctionSource value={text()} />
      <NamespaceOmit value={text()} />
      <NestedViews value={text()} />
      <AliasThenOmit value={text()} />
      <IntrinsicSpread value={text()} />
      <MergeSingleSource value={text()} />
      <MergeMaybeSource value={text()} />
      <MergeSpreadSource value={text()} />
      <AliasWrite value={text()} />
      <AliasEscape value={text()} />
      <Destructured value={text()} />
      <ShadowedOmit value={text()} />
      <IntrinsicSpreadWrite value={text()} />
      <OmitThenWrite value={text()} />
      <ComponentSpread value={text()} />
      <ReadThroughView value={text()} />
      <StaticOmit value="fixed" />
    </div>
  );
}
