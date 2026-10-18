import { createEffect, createMemo } from "solid-js";

// Project callees that run a caller's literal only through one parameter.

type Keyed = { key: () => string; other: number };

// Only inside a memo's tracked compute.
export function memoOf(options: Keyed) {
  return createMemo(() => options.key() + options.other);
}

// Only inside an effect's tracked compute.
export function effectOf(options: { key: () => string }) {
  createEffect(
    () => options.key(),
    (key) => {
      console.log(key);
    },
  );
}

// Only from a host timer, after the call returns.
export function laterOf(options: { run: () => void }) {
  setTimeout(() => options.run(), 0);
}

// Inside an effect's apply callback, which does not track.
export function applyOf(options: { key: () => string }) {
  createEffect(
    () => 1,
    () => {
      console.log(options.key());
    },
  );
}

// In the callee's own body, during the call.
export function bodyOf(options: { key: () => string }) {
  return options.key();
}

// In a tracked compute and from a timer: two roles. The optional `extra`
// call keeps the per-call-site summary path from deciding this case.
export function mixedOf(options: { key: () => string; extra?: () => void }) {
  const memo = createMemo(() => options.key());
  setTimeout(() => options.key(), 0);
  return () => {
    options.extra?.();
    return memo();
  };
}

// Handed on to an unknown sink as well.
export function escapeOf(options: { key: () => string }, sink: (read: () => string) => void) {
  sink(options.key);
  return createMemo(() => options.key());
}

// A callback that writes, run in a tracked compute.
export function writeOf(options: { bump: () => void }) {
  return createMemo(() => {
    options.bump();
    return 1;
  });
}

// A component that calls its prop only in tracked JSX.
export function Label(props: { text: () => string }) {
  return <span>{props.text()}</span>;
}

// A component that tests its prop and forwards it to `Label`.
export function Outer(props: { text?: () => string }) {
  return props.text ? <Label text={props.text} /> : <span>none</span>;
}

// A component that spreads its props on.
export function Spread(props: { text: () => string }) {
  return <Label {...props} />;
}

// Like a modal-focus hook: `active` runs only in an effect's tracked compute,
// `onEscape` only from the returned handler.
export function modalOf(options: {
  active: () => boolean;
  onEscape: () => void;
  ignore?: (event: KeyboardEvent) => boolean;
}) {
  createEffect(
    () => options.active(),
    (active) => {
      console.log(active);
    },
  );
  return (event: KeyboardEvent) => {
    if (options.ignore?.(event)) return;
    if (event.key === "Escape") options.onEscape();
  };
}

// Like a virtualizer: `count` in a memo, `version` in a timer, `rows` returned.
export function listOf(options: { count: () => number; version: () => string; rows: () => number[] }) {
  const size = createMemo(() => options.count());
  setTimeout(() => console.log(options.version()), 0);
  return { size, rows: () => options.rows() };
}
