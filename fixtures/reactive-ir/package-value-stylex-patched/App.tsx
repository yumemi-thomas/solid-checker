import * as stylex from "@stylexjs/stylex";
import { defineVars } from "@stylexjs/stylex";
import { create as otherCreate } from "other-styles";

// ADR 1063: in @stylexjs/stylex 0.19.0's runtime (read, pinned by digest), `create` and `defineVars` throw on every call ("Styles must be compiled by
// '@stylexjs/babel-plugin'"): neither returns a value, so nothing read off their results runs, and no package-value-read row stands for those reads.
const tokens = defineVars({ gap: "4px" });
const styles = stylex.create({ base: { padding: tokens.gap } });

// No row: a read of `create`'s result in a component body, and a read of `defineVars`'s result.
export function Created() {
  const base = styles.base;
  return <p>{String(base)}</p>;
}
export function Tokens() {
  const gap = tokens.gap;
  return <p>{gap}</p>;
}

// Rows stay: `attrs` returns a fresh object (no reading says what), and another package's `create` returns what it is handed.
export function Attrs() {
  const attributes = stylex.attrs(styles.base);
  const name = attributes.class;
  return <p>{name}</p>;
}
const other = otherCreate({ base: { padding: "4px" } });
export function OtherPackage() {
  const base = other.base;
  return <p>{base.padding}</p>;
}
