import { createSignal } from "solid-js";
import { useInner, useLabel } from "dep-hooks";
import { readArray } from "dep-array";
import { readExt } from "dep-ext";
import { readPattern } from "dep-pattern/read";
import { readPlain } from "plain-utils";

// Positive: a Solid package the project imports is selected.
export function LabelNow() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}

// Positive: a Solid package that package imports is selected too, resolved
// from the importing package's own node_modules.
export function Inner() {
  const [name] = createSignal("a");
  const label = useInner(name);
  return <p>{label}</p>;
}

// Uncertifiable: a package that is not a Solid package is not selected, and
// stays an unknown package.
export function Plain() {
  const [name] = createSignal("a");
  const label = readPlain(name);
  return <p>{label}</p>;
}

// Positive: a subpath pattern (`./*`) resolves as Node resolves it.
export function Pattern() {
  const [name] = createSignal("a");
  const label = readPattern(name);
  return <p>{label}</p>;
}

// Uncertifiable: a Solid package whose `exports` this cannot read (an array
// fallback) is skipped with a note, and stays an unknown package.
export function ArrayExports() {
  const [name] = createSignal("a");
  const label = readArray(name);
  return <p>{label}</p>;
}

// Positive: `./read` resolves to `read.mjs`, Vite's first extension, whose
// `readNow` reads the accessor (review round 1 of ADR 0290, Sol).
export function ExtensionOrder() {
  const [name] = createSignal("a");
  const label = readExt(name);
  return <p>{label}</p>;
}
