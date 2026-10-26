import { createSignal } from "solid-js";

// Each component below holds one untracked read (SC1001, strict-read-untracked)
// at the line `console.log(doubled());`, guarded by one suppression form
// (ADR 0429). `Plain` has no comment and is the one reported site.

export function Plain() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled());
  return <div>static</div>;
}

export function EslintNextLine() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/strict-read-untracked
  console.log(doubled());
  return <div>static</div>;
}

export function EslintLine() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled()); // eslint-disable-line solid-checker/strict-read-untracked
  return <div>static</div>;
}

export function EslintBlockLine() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  /* eslint-disable-next-line solid-checker/strict-read-untracked -- accepted: logged once */
  console.log(doubled());
  return <div>static</div>;
}

export function UpstreamGroupName() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid/reactivity
  console.log(doubled());
  return <div>static</div>;
}

export function CheckerGroupName() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/reactivity
  console.log(doubled());
  return <div>static</div>;
}

export function NameListWithOtherRule() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line solid-checker/uncalled-accessor, solid-checker/strict-read-untracked
  console.log(doubled());
  return <div>static</div>;
}

export function CheckerFamilyBareName() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // solid-checker-disable-next-line strict-read-untracked
  console.log(doubled());
  return <div>static</div>;
}

export function CheckerFamilyCode() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled()); // solid-checker-disable-line SC1001
  return <div>static</div>;
}

export function CheckerFamilyPrefixed() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // solid-checker-disable-next-line solid-checker/strict-read-untracked
  console.log(doubled());
  return <div>static</div>;
}

export function BareDisablesEveryRule() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  // eslint-disable-next-line
  console.log(doubled());
  return <div>static</div>;
}

// A block disable runs until its enable; the site after it is reported.
export function BlockRange() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  /* eslint-disable solid-checker/strict-read-untracked */
  console.log(doubled());
  console.log(doubled());
  /* eslint-enable solid-checker/strict-read-untracked */
  console.log(doubled());
  return <div>static</div>;
}

// The checker's own family has the same block form. A bare disable covers every
// rule, an enable naming one rule switches only that rule back on, and a bare
// enable ends the rest (without it the disable would run to the end of the file).
export function CheckerBlockRange() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  /* solid-checker-disable */
  console.log(doubled());
  /* solid-checker-enable strict-read-untracked */
  console.log(doubled());
  /* solid-checker-enable */
  return <div>static</div>;
}

// The group disables its member (SC1007) inside JSX.
export function JsxComment() {
  const numberHandler = 1;
  return (
    <div>
      {/* eslint-disable-next-line solid-checker/reactive-handler-frozen */}
      <button onCustom-event={numberHandler}>suppressed</button>
      <button onCustom-event={numberHandler}>reported</button>
    </div>
  );
}
