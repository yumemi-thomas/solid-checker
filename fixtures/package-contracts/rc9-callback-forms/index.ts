import { omit, until, untrack } from "solid-js";
import { dynamic, type DynamicOptions, type ValidComponent } from "@solidjs/web";

// Positive: `dynamic(source, { static: true })` is `untrack(source)` at the
// call (`@solidjs/web@2.0.0-rc.9` `dist/web.dev.js:2199`), so the forwarded
// source has run, untracked, before the export returns: `inline`, clearing
// the listener -- the same row `untrackedSource` publishes.
export function staticSource<T extends ValidComponent>(source: () => T) {
  return dynamic(source, { static: true });
}

// Reference: the `untrack` the static form is.
export function untrackedSource<T>(source: () => T): T {
  return untrack(source);
}

// Control: the default form is unchanged -- the source is the tracked compute
// of the memo `dynamic` builds.
export function trackedSource<T extends ValidComponent>(source: () => T) {
  return dynamic(source);
}

// Negative: the options value is the caller's, so the runtime may take either
// form. No word is published; the unknown-callback sentinel opens instead.
export function optionedSource<T extends ValidComponent>(
  source: () => T,
  options: DynamicOptions
) {
  return dynamic(source, options);
}

// Negative: rc.9's `omit(props, hidden)` calls `hidden` on every read of the
// view it returns (`@solidjs/signals@2.0.0-rc.9` `dist/dev.js:3495-3498`,
// `:4178-4241`), which no execution word states. It used to be published as a
// value the export never invokes.
export function hiddenBy<T extends Record<string, unknown>>(
  props: T,
  hidden: (key: keyof T & (string | symbol)) => boolean
) {
  return omit(props, hidden);
}

// Control: a key list is a value, as before.
export function withoutA<T extends { a: unknown }>(props: T) {
  return omit(props, "a");
}

// Negative: `until`'s predicate runs during the call, again on every change
// until it is truthy, and not at all behind an aborted signal
// (`@solidjs/signals@2.0.0-rc.9` `dist/dev.js:2717-2785`) -- `resolve`'s shape,
// and like `resolve` it gets no word.
export function whenReady<T>(predicate: () => T) {
  return until(predicate);
}
