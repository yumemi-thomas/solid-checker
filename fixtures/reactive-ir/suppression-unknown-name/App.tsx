import { createSignal } from "solid-js";

// SC9016 (unknown-suppression-rule, ADR 0429): a `solid-checker/*` name nothing
// resolves is a project notice, once per distinct name at its first use, and
// the finding it was meant to suppress is still reported.

export function Typo() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/strict-read-untrackd
  console.log(doubled());
  return <div>static</div>;
}

export function TypoAgain() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/strict-read-untrackd, solid-checker/strict-read-untracked
  console.log(doubled());
  return <div>static</div>;
}

// An SC code is a `solid-checker-*` spelling: ESLint refuses it as a rule id.
export function CodeInEslintFamily() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/SC1001
  console.log(doubled());
  return <div>static</div>;
}

export function UnknownCode() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // solid-checker-disable-next-line SC1999
  console.log(doubled());
  return <div>static</div>;
}

// Accepted silently: the adapter's ids, eslint-plugin-solid's pinned names
// (with or without a checker rule), retired identities, former names and other
// plugins' names. The obligations SC9005 and SC9012 and their former names and
// codes (ADR 0440) are among them (last line of the file): no comment
// suppresses an obligation. None is noticed and none suppresses the read below.
export function Silent() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/certification, solid-checker/contract-note, solid-checker/package-note, solid/imports, solid/no-single-arg-create-effect, solid/prefer-classlist, solid-checker/untracked-derived-function, solid-checker/v1/imports, solid-checker/no-owner-cleanup, solid-checker/package-contract-missing, solid-checker/package-contract-incomplete, solid-checker/package-behavior-unavailable, solid-checker/reactive-dispatch-unresolved, solid-checker/reactive-source-uncaptured, react-hooks/exhaustive-deps
  console.log(doubled());
  return <div>static</div>;
}

/* solid-checker-disable SC9005, SC9011, SC9012, package-behavior-unavailable, reactive-dispatch-unresolved */
