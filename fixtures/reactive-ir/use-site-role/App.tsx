import { createSignal } from "solid-js";
import {
  Label,
  Outer,
  Spread,
  applyOf,
  bodyOf,
  effectOf,
  escapeOf,
  laterOf,
  listOf,
  memoOf,
  modalOf,
  mixedOf,
  writeOf,
} from "./hooks";
import { request } from "./state";

const base = { other: 2 };

// ---- Clean: the literal runs only where the callee's calls of it run.

export function UsesMemo() {
  const [count] = createSignal(0);
  const label = memoOf({ key: () => String(count()), other: 1 });
  return <main>{label()}</main>;
}
export function UsesEffectCompute() {
  const [count] = createSignal(0);
  effectOf({ key: () => String(count()) });
  return <main>effect</main>;
}
export function UsesDeferred() {
  const [count] = createSignal(0);
  laterOf({ run: () => console.log(count()) });
  return <main>later</main>;
}
export function UsesLabel() {
  const [count] = createSignal(0);
  return <Label text={() => String(count())} />;
}
export function UsesOuter() {
  const [count] = createSignal(0);
  return <Outer text={() => String(count())} />;
}

export function UsesModal() {
  const [open] = createSignal(false);
  const [count] = createSignal(0);
  const onKey = modalOf({
    active: () => open(),
    onEscape: () => console.log(count()),
    ignore: () => false,
  });
  return <div onKeyDown={onKey}>modal</div>;
}
export function UsesModuleState() {
  const onKey = modalOf({ active: () => request() != null, onEscape: () => console.log("escape") });
  const label = memoOf({ key: () => request() ?? "", other: 1 });
  effectOf({ key: () => request() ?? "" });
  laterOf({ run: () => console.log(request()) });
  return <div onKeyDown={onKey}>{label()}</div>;
}
export function UsesList() {
  const [count] = createSignal(0);
  const [version] = createSignal("a");
  const [rows] = createSignal<number[]>([]);
  const list = listOf({ count: () => count(), version: () => version(), rows: () => rows() });
  return <main>{list.size()}</main>;
}

// ---- Uncertifiable: no single proven role.

export function UsesApply() {
  const [count] = createSignal(0);
  applyOf({ key: () => String(count()) });
  return <main>apply</main>;
}
export function UsesBody() {
  const [count] = createSignal(0);
  const value = bodyOf({ key: () => String(count()) });
  return <main>{value}</main>;
}
export function UsesMixed() {
  const [count] = createSignal(0);
  const label = mixedOf({ key: () => String(count()) });
  return <main>{label()}</main>;
}
export function UsesGetter() {
  const [count] = createSignal(0);
  const label = memoOf({
    key: () => String(count()),
    get other() {
      return 1;
    },
  });
  return <main>{label()}</main>;
}
export function UsesSpreadLiteral() {
  const [count] = createSignal(0);
  const label = memoOf({ ...base, key: () => String(count()) });
  return <main>{label()}</main>;
}
export function UsesEscape() {
  const [count] = createSignal(0);
  const label = escapeOf({ key: () => String(count()) }, (read) => console.log(read));
  return <main>{label()}</main>;
}
export function UsesSpreadProps() {
  const [count] = createSignal(0);
  return <Spread text={() => String(count())} />;
}

// ---- Write legality keeps its role: the write in a tracked compute.

export function UsesWrite() {
  const [, setCount] = createSignal(0);
  const value = writeOf({ bump: () => setCount(1) });
  return <main>{value()}</main>;
}
