import { createSignal } from "solid-js";

// The census gap. The 1.x compiler drops a nested, non-hydratable `<head>`
// while lowering -- the generated browser HTML for this component is
// `<div><title></title></div>` -- and its execution census drops the whole
// head range with it. Nothing in the execution map mentions `{title()}`.
//
// Absence of a census entry is not a fact. The read is therefore an
// uncertifiable proof obligation: the compiler never said this JSX expression
// is untracked, and it never said the expression was deleted either. Reporting
// a violation here would be a proven claim about code the compiler declined to
// report on.
function ReadInsideDroppedHead() {
  const [title] = createSignal("t");
  return (
    <div>
      <head>
        <title>{title()}</title>
      </head>
    </div>
  );
}

// The same gap reached through an attribute expression container rather than a
// child, so both arms of the source-level JSX-region lookup are pinned.
function AttributeReadInsideDroppedHead() {
  const [id] = createSignal("i");
  return (
    <div>
      <head>
        <title id={id()} />
      </head>
    </div>
  );
}

// Censused, then retracted. The static-template `<noscript>` fast path emits
// the inert tag and skips its children wholesale. At `d1e08958` the producer
// began withdrawing every site in that discarded range instead of rejecting
// the whole file during trace reconciliation. The resulting source/census hole
// is an uncertifiable proof obligation, exactly like the dropped-head paths.
function ReadInsideDiscardedNoscript() {
  const [note] = createSignal("n");
  return <div><noscript>{note()}</noscript></div>;
}

// Negative: an ordinary tracked child. The census records it, the compiler
// proves it re-runs, and the checker stays silent. This is what the census-gap
// escalation must not start reporting.
function TrackedChildStaysCertified() {
  const [name] = createSignal("n");
  return (
    <div>
      <span>{name()}</span>
    </div>
  );
}

// Negative: an untracked read whose proof owes nothing to the census. The read
// is not inside any JSX expression at all, so the untracked-rendering role
// rests on the component body alone and SC1001 stays a proven violation --
// exactly as before this escalation existed.
function ReadOutsideJsxStaysAViolation() {
  const [count] = createSignal(0);
  const snapshot = count();
  return (
    <div>
      <span>{snapshot}</span>
    </div>
  );
}

// One exported root so every component above has a call site the analyzer can
// enumerate. Without it each read would be uncertifiable for the unrelated
// reason that the component's callers are unknown, and the fixture would pass
// while proving nothing about the census.
export function Root() {
  return (
    <div>
      <ReadInsideDroppedHead />
      <AttributeReadInsideDroppedHead />
      <ReadInsideDiscardedNoscript />
      <TrackedChildStaysCertified />
      <ReadOutsideJsxStaysAViolation />
    </div>
  );
}
