import * as S from "solid-js";
import { createMemo, createRoot, createTrackedEffect, onSettled } from "solid-js";

// ADR 1009: a function that is a plain property of a `const` object literal nothing else reaches, handed to a primitive as
// `bundle.key`, runs where that primitive runs its callback.

// Violations: SC5001, a pending read in a leaf owner (`onSettled`, `createTrackedEffect`).
export function Settled() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { user(); } };
  onSettled(bundle.run);
  return <p />;
}

export function Tracked() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { user(); } };
  createTrackedEffect(bundle.run);
  return <p />;
}

export function NestedClosure() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => {
    const inner = () => { user(); };
    inner();
  } };
  onSettled(bundle.run);
  return <p />;
}

export function NestedLiteral() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { (() => { user(); })(); } };
  onSettled(bundle.run);
  return <p />;
}

export function TwoProperties() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { user(); }, label: "x" };
  onSettled(bundle.run);
  return <p />;
}

// Obligations: something else can reach or change the object, or the property is not a plain data property. Each case has a
// key of its own: a write of a key anywhere in the file opens every member of that name.
function keep(value: unknown) {
  void value;
}

export function ObjectPassed() {
  const user = createMemo(async () => 1);
  const bundle = { passed: () => { user(); } };
  keep(bundle);
  onSettled(bundle.passed);
  return <p />;
}

export function PropertyWritten() {
  const user = createMemo(async () => 1);
  const bundle = { written: () => { user(); } };
  bundle.written = () => {};
  onSettled(bundle.written);
  return <p />;
}

export function ComputedKey() {
  const user = createMemo(async () => 1);
  const key = "computed" as const;
  const bundle = { [key]: () => { user(); } };
  onSettled(bundle[key]);
  return <p />;
}

export function LetObject() {
  const user = createMemo(async () => 1);
  let bundle = { loose: () => { user(); } };
  onSettled(bundle.loose);
  return <p />;
}

export function Method() {
  const user = createMemo(async () => 1);
  const bundle = { method() { user(); } };
  onSettled(bundle.method);
  return <p />;
}

export function MemberAlsoCalled() {
  const user = createMemo(async () => 1);
  const bundle = { called: () => { user(); } };
  bundle.called();
  onSettled(bundle.called);
  return <p />;
}

export function MemberAliased() {
  const user = createMemo(async () => 1);
  const bundle = { aliased: () => { user(); } };
  const held = bundle.aliased;
  onSettled(held);
  return <p />;
}

export function NestedBundle() {
  const user = createMemo(async () => 1);
  const bundle = { inner: { deep: () => { user(); } } };
  onSettled(bundle.inner.deep);
  return <p />;
}

// ADR 1009 review F1: the property must be the key's FINAL value. A later spread, a later key with no static name or a later
// property of the same key may replace it: the primitive then runs another function (Chrome is silent in the first two).
export function SpreadAfter() {
  const user = createMemo(async () => 1);
  const extra: { spreadAfter?: () => void } = { spreadAfter: () => {} };
  const bundle = { spreadAfter: () => { user(); }, ...extra };
  onSettled(bundle.spreadAfter);
  return <p />;
}

export function ComputedAfter() {
  const user = createMemo(async () => 1);
  const k: string = "computedAfter";
  const bundle = { computedAfter: () => { user(); }, [k]: () => {} };
  onSettled(bundle.computedAfter);
  return <p />;
}

// The same shapes with the spread or the computed key BEFORE the property: it is the final value and Chrome reports the read, but the
// read is not proved (an obligation: a literal with a spread or a computed key is not read). A known miss, kept closed.
export function SpreadBefore() {
  const user = createMemo(async () => 1);
  const extra: { other?: () => void } = {};
  const bundle = { ...extra, spreadBefore: () => { user(); } };
  onSettled(bundle.spreadBefore);
  return <p />;
}

export function ComputedBefore() {
  const user = createMemo(async () => 1);
  const k: string = "computedBefore";
  const bundle = { [k]: () => {}, computedBefore: () => { user(); } };
  onSettled(bundle.computedBefore);
  return <p />;
}

// Review F2. Written the same way, the hand-off is a primitive's only when the callee is the dialect's primitive.
export function ShadowedLocal() {
  const user = createMemo(async () => 1);
  const onSettled = (f: () => void) => { void f; };
  const bundle = { shadowLocal: () => { user(); } };
  onSettled(bundle.shadowLocal);
  return <p />;
}

function shadowParam(onSettled: (f: () => void) => void) {
  const user = createMemo(async () => 1);
  const bundle = { shadowParam: () => { user(); } };
  onSettled(bundle.shadowParam);
}

export function ShadowedParameter() {
  shadowParam((f) => { void f; });
  return <p />;
}

export function NotPrimitive() {
  const user = createMemo(async () => 1);
  const bundle = { notPrimitive: () => { user(); } };
  keep(bundle.notPrimitive);
  return <p />;
}

export function WrappedInArrow() {
  const user = createMemo(async () => 1);
  const bundle = { wrapped: () => { user(); } };
  onSettled(() => bundle.wrapped());
  return <p />;
}

export function QuotedKey() {
  const user = createMemo(async () => 1);
  const bundle = { "quoted": () => { user(); } };
  onSettled(bundle.quoted);
  return <p />;
}

export function Destructured() {
  const user = createMemo(async () => 1);
  const bundle = { destructured: () => { user(); } };
  const { destructured } = bundle;
  onSettled(destructured);
  return <p />;
}

// Module-level bundles: exported by a declaration, by a specifier with another name, as the default; and one nothing exports.
const shared = createRoot(() => createMemo(async () => 1));
export const exportedBundle = { exportedDecl: () => { shared(); } };
const specBundle = { exportedSpec: () => { shared(); } };
export { specBundle as renamedBundle };
const defaultBundle = { exportedDefault: () => { shared(); } };
export default defaultBundle;
const moduleBundle = { moduleOnly: () => { shared(); } };

export function ModuleUses() {
  onSettled(exportedBundle.exportedDecl);
  onSettled(specBundle.exportedSpec);
  onSettled(defaultBundle.exportedDefault);
  return <p />;
}

// A plain module-level bundle nothing exports: Chrome reports the read, the checker keeps the obligation (a module-level bundle is not read).
export function ModuleClosed() {
  onSettled(moduleBundle.moduleOnly);
  return <p />;
}

// Hand-offs written through TypeScript sugar and a namespace import of the dialect's module (violations).
export function NonNull() {
  const user = createMemo(async () => 1);
  const bundle = { nonNull: () => { user(); } };
  onSettled(bundle.nonNull!);
  return <p />;
}

export function CastAny() {
  const user = createMemo(async () => 1);
  const bundle = { castAny: () => { user(); } };
  onSettled(bundle.castAny as any);
  return <p />;
}

export function NamespaceCall() {
  const user = createMemo(async () => 1);
  const bundle = { nsCall: () => { user(); } };
  S.onSettled(bundle.nsCall);
  return <p />;
}

// No violation: the same bundle handed to a tracked compute, which may suspend.
export function TrackedCompute() {
  const user = createMemo(async () => 1);
  const bundle = { compute: () => user() };
  const doubled = createMemo(bundle.compute);
  return <p>{doubled()}</p>;
}
