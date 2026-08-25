import { createSignal } from "solid-js";

// The census gap. A void element cannot hold children in HTML, so the 2.0
// compiler emits no execution site for `{count()}` and the generated code
// never reads `count`. Nothing in the execution map mentions the expression.
//
// Absence of a census entry is not a fact. The read is therefore an
// uncertifiable proof obligation: the compiler never said this JSX expression
// is untracked, and it never said the expression was dropped either. Reporting
// a violation here would be a proven claim about code the compiler declined to
// report on -- and certifying it safe would be a second claim with no more
// evidence behind it.
//
// The `<br>` stays at the root of its own component on purpose, and that is
// what makes this the census gap rather than the divergence. At template root
// the producer gates child lowering on `!is_void_element` and emits nothing,
// exactly as the compiler Solid ships does -- the two agree, and the hole is a
// genuine hole. *Nested* (`<div><br>{count()}</br></div>`) the producer really
// lowers the child and reports a `reactive-rerun` site the shipped compiler
// would never emit; that is a different claim with a different mitigation, and
// fixtures/reactive-ir/jsx-void-child-divergence-solid-2 pins it.
function ReadInsideVoidElementChild() {
  const [count] = createSignal(0);
  return <br>{count()}</br>;
}

// Resolved control. The current producer matches Babel's `!hasChildren` gate:
// with a real child it emits the insert and makes the textContent effect target
// that child's text node. `{body()}` therefore has an explicit tracked site and
// SC1001 stays silent. Keeping the authoring conflict beside it proves that the
// pin move removes only the obsolete census-gap claim.
//
// SC8003 fires on the same element for an unrelated, legitimate reason: JSX
// children and `textContent` at once is a conflict the author can see. Two
// claims about one element, neither duplicating the other.
function TextContentChildNowCertified() {
  const [label] = createSignal("l");
  const [body] = createSignal("b");
  return <div><span textContent={label()}>{body()}</span></div>;
}

// The retraction way a hole arrives, and the negative control for the `<noscript>`
// divergence mitigation. `<noscript>`'s markup is inert, so on the
// static-template fast path this producer emits the tag and returns without
// visiting the children at all; the recorder retracts every site in the
// unvisited subtree. Babel drops the same subtree, so here the two compilers
// *agree* and the retraction is parity-clean -- an ordinary census gap.
//
// That makes this case load-bearing twice over. It pins the gap wording, and it
// is what fails if `divergent_lowered_child` ever keys on the `<noscript>` tag
// alone instead of on a lowered site: this arm would flip to the divergence
// wording, and this fixture is in coverage's `KEEPS_WORDING` set, so the flip
// cannot pass silently. The divergence pair holds the two positions where this
// producer *does* keep the children (template root, and off the fast path via a
// dynamic attribute).
//
// The 1.x producer learned the same retraction at `d1e08958`; its focused
// census-gap fixture now pins that dialect's arm too.
function RetractedInertNoscriptChild() {
  const [note] = createSignal("n");
  return <div><noscript>{note()}</noscript></div>;
}

// Negative: an ordinary tracked child. The census records it, the compiler
// proves it re-runs, and the checker stays silent. This is what the census-gap
// escalation must not start reporting.
function TrackedChildStaysCertified() {
  const [name] = createSignal("n");
  return <span>{name()}</span>;
}

// Negative: an untracked read whose proof owes nothing to the census. The read
// is not inside any JSX expression at all, so the untracked-rendering role
// rests on the component body alone and SC1001 stays a proven violation --
// exactly as before this escalation existed.
function ReadOutsideJsxStaysAViolation() {
  const [total] = createSignal(0);
  const snapshot = total();
  return <span>{snapshot}</span>;
}

// One exported root giving every component above an enumerable call site.
//
// Measured, not assumed: deleting it leaves this fixture's findings
// byte-identical, so it is *not* load-bearing here. It is kept for symmetry
// with the 1.x sibling, where the same deletion flips
// `ReadOutsideJsxStaysAViolation` from a proven violation to an uncertifiable
// finding -- for a reason unrelated to the census -- and so destroys that
// fixture's second negative. Keeping both roots means the pair differs only in
// the shape its producer declines to census.
export function Root() {
  return (
    <div>
      <ReadInsideVoidElementChild />
      <TextContentChildNowCertified />
      <RetractedInertNoscriptChild />
      <TrackedChildStaysCertified />
      <ReadOutsideJsxStaysAViolation />
    </div>
  );
}
