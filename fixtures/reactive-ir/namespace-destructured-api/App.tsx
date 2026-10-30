// ADR 0459: a `const` object destructuring of a namespace import binding, with
// a static key and a bare binding, holds exactly the export `S.key` reads, so
// the slot names that primitive wherever it is called. Every other shape stays
// unresolved, which is never a violation.
import * as S from "solid-js";

type Theme = "light" | "dark";
const ThemeContext = S.createContext<Theme>();

const { onCleanup: cleanup, "useContext": readContext, createEffect, Show: ShowD } = S;
// A TypeScript wrapper around the initializer is the same namespace.
const { onCleanup: wrappedCleanup } = S as typeof S;

// Uncertifiable, not a violation: `early` runs before the declarator below,
// so this mention throws instead of calling `onCleanup`.
function early() {
  lateCleanup(() => {});
}
try {
  early();
} catch {}
const { onCleanup: lateCleanup } = S;

// Positive: a timer callback runs with no owner.
export function TimerCleanup() {
  setTimeout(() => cleanup(() => {}), 0);
  return <i>timer</i>;
}

// Positive: an event handler has no owner.
export function HandlerContext() {
  return <button onClick={() => readContext(ThemeContext)}>theme</button>;
}

// Positive: an effect created in an event handler has no owner.
export function HandlerEffect() {
  return <button onClick={() => createEffect(() => 1, () => {})}>effect</button>;
}

// Positive: a destructuring in a function body is the same export.
export function BodyDestructuring() {
  const { onCleanup: localCleanup } = S;
  setTimeout(() => localCleanup(() => {}), 0);
  return <i>body</i>;
}

// Positive: the wrapped initializer's slot in a timer.
export function WrappedTimer() {
  setTimeout(() => wrappedCleanup(() => {}), 0);
  return <i>wrapped</i>;
}

// Uncertifiable, then a violation: inside a timer callback, `early` runs
// before the body's declarator (the read throws); the call after it runs
// with no owner.
export function BodyTdz() {
  setTimeout(() => {
    function early() {
      bodyLate(() => {});
    }
    try {
      early();
    } catch {}
    const { onCleanup: bodyLate } = S;
    bodyLate(() => {});
  }, 0);
  return <i>body tdz</i>;
}

// Negative: the same slots in a component body, which has an owner, and a
// destructured `Show` tag whose children callback reads the narrowed value.
export function Owned() {
  cleanup(() => {});
  createEffect(() => 1, () => {});
  const theme = readContext(ThemeContext);
  return <ShowD when={theme}>{(current: () => Theme) => <i>{current()}</i>}</ShowD>;
}

// Negative: a parameter named `S` shadows the namespace.
export function shadowParameter(S: { onCleanup(fn: () => void): void }): void {
  const { onCleanup: shadowed } = S;
  setTimeout(() => shadowed(() => {}), 0);
}

// Negative: a local object named `S` shadows the namespace.
export function ShadowLocal() {
  const S = { onCleanup(fn: () => void) { fn(); } };
  const { onCleanup: shadowed } = S;
  setTimeout(() => shadowed(() => {}), 0);
  return <i>local</i>;
}

// Negative: a computed key, a default value, a rest element and a `let`
// destructuring bind no single export this resolution names.
const { ["onCleanup"]: computedCleanup } = S;
const { onCleanup: defaultedCleanup = (fn: () => void) => fn } = S;
const { createMemo: _memo, ...rest } = S;
let { onCleanup: reassignableCleanup } = S;
export function Refused() {
  setTimeout(() => {
    computedCleanup(() => {});
    defaultedCleanup(() => {});
    rest.onCleanup(() => {});
    reassignableCleanup(() => {});
  }, 0);
  return <i>refused</i>;
}

// Negative: a slot the module exports (a declaration, a specifier, the default)
// is referenced from other modules too; it is not resolved.
export const { onCleanup: exportedCleanup } = S;
const { onCleanup: specifiedCleanup } = S;
const { onCleanup: defaultCleanup } = S;
export { specifiedCleanup };
export default defaultCleanup;
export function ExportedSlot() {
  setTimeout(() => {
    exportedCleanup(() => {});
    specifiedCleanup(() => {});
    defaultCleanup(() => {});
  }, 0);
  return <i>exported</i>;
}

// ADR 0515: a slot over a `const` holding the namespace is the namespace's.
const HeldS = S;
const { onCleanup: heldCleanup } = HeldS;

export function HeldTimer() {
  setTimeout(() => heldCleanup(() => {}), 0);
  return <div />;
}

export function HeldOwned() {
  heldCleanup(() => {});
  return <div />;
}
