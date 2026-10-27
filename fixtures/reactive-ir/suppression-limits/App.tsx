import { createSignal } from "solid-js";

// What a suppression comment does not do (ADR 0429). Every component holds one
// untracked read (SC1001) at `console.log(doubled());`, and every one is still
// reported; the comment above it is not a suppression for that read.

export function ForeignRuleName() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line prefer-rest-params, @typescript-eslint/no-unused-vars
  console.log(doubled());
  return <div>static</div>;
}

export function OtherCheckerRule() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/uncalled-accessor
  console.log(doubled());
  return <div>static</div>;
}

export function NotTheNextLine() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/strict-read-untracked

  console.log(doubled());
  return <div>static</div>;
}

// An upstream name without a checker rule suppresses nothing, with no notice.
export function UpstreamNameWithoutRule() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid/no-single-arg-create-effect, solid/some-other-plugins-rule
  console.log(doubled());
  return <div>static</div>;
}

// A bare name in an `eslint-disable` comment is ESLint's or another plugin's.
export function UnprefixedEslintName() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line strict-read-untracked
  console.log(doubled());
  return <div>static</div>;
}

// ESLint reads a `//` comment only as a line form, so this is no block disable.
// eslint-disable solid-checker/strict-read-untracked
export function LineCommentBlockForm() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled());
  return <div>static</div>;
}

// A `-line` comment must stay on one line.
export function MultiLineLineComment() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  /* eslint-disable-line solid-checker/strict-read-untracked --
     spans two lines */ console.log(doubled());
  return <div>static</div>;
}

// Names are separated by commas: a space makes one unknown name, noticed as
// SC9016, and the site is reported.
export function SpaceSeparatedNames() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // solid-checker-disable-next-line strict-read-untracked uncalled-accessor
  console.log(doubled());
  return <div>static</div>;
}

// An obligation is not a defect the author can accept: a callable-or-number
// handler is uncertifiable (SC1007), and a bare comment covering every rule
// leaves it in place.
declare const maybeHandler: (() => void) | number;

export function UncertifiableStaysUncertifiable() {
  return (
    <div>
      {/* eslint-disable-next-line */}
      <button onCustom-maybe={maybeHandler}>uncertain</button>
    </div>
  );
}
