import { onCleanup } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import * as styler from "fake-styler";
import { attrs, passThrough } from "fake-styler";
import { attrs as declaredAttrs } from "declared-only";

// ADR 0372: every return of `attrs` is an object literal keyed `class` or
// `style`, so the spread hands no handler: no obligation at the spread, and
// the click handler before it is not replaced (ADR 0364), so its `onCleanup`
// with no owner is proven.
export function NamespaceAttrs() {
  return <div onClick={() => onCleanup(() => {})} {...styler.attrs("a")} />;
}

// The same export through a named import binding.
export function NamedAttrs() {
  return <div onClick={() => onCleanup(() => {})} {...attrs("a")} />;
}

// Published `@stylexjs/stylex@0.19.0`: `attrs` builds a `const result = {}`
// and writes `class`, `style` and `data-style-src` into it.
export function Stylex() {
  return <div onClick={() => onCleanup(() => {})} {...stylex.attrs()} />;
}

// Obligation: `passThrough` returns its parameter, which may hold `onClick`.
export function ParameterReturn() {
  return <div onClick={() => onCleanup(() => {})} {...passThrough({ class: "a" })} />;
}

// Obligation: `keyed` returns a computed key.
export function ComputedKey() {
  return <div onClick={() => onCleanup(() => {})} {...styler.keyed("onClick")} />;
}

// Obligation: `withHandler` returns an `onClick` key.
export function EventKey() {
  return <div onClick={() => onCleanup(() => {})} {...styler.withHandler("a")} />;
}

// Obligation: `declared-only` ships a declaration and no runtime source.
export function DeclarationOnly() {
  return <div onClick={() => onCleanup(() => {})} {...declaredAttrs("a")} />;
}

// Obligation: a local `styler` shadows the namespace import, so the call is
// not the package's.
export function Shadowed() {
  const styler = { attrs: (value: object) => value };
  return <div onClick={() => onCleanup(() => {})} {...styler.attrs({ class: "a" })} />;
}

// Obligation: the package module rewrites `rewritten` after declaring it, so
// the declaration's returns are not what the call gets.
export function RewrittenExport() {
  return <div onClick={() => onCleanup(() => {})} {...styler.rewritten("a")} />;
}
