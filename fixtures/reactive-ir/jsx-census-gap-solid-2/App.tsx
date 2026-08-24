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

// The retraction arm of the same gap, and the reason the mitigation cannot key
// on "the compiler emitted no site for a shape we recognize": here the
// producer *did* census `{y()}` and then retracted it during lowering, because
// its nested dynamic-`textContent` path replaces the element's content with a
// text placeholder and discards the source children. What reaches the checker
// is a hole in exactly the same shape as above, arrived at from the opposite
// direction -- so it takes the same wording and the same verdict.
//
// The shipped compiler diverges here too (it emits no placeholder with
// children present, inserts `y`, and writes `.data` into whatever the insert
// produced), which is why the retraction may not be read as no-execution.
// Uncertifiable is the only honest answer either way.
//
// SC8003 fires on the same element for an unrelated, legitimate reason: JSX
// children and `textContent` at once is a conflict the author can see. Two
// claims about one element, neither duplicating the other.
function RetractedTextContentShadowedChild() {
  const [label] = createSignal("l");
  const [body] = createSignal("b");
  return <div><span textContent={label()}>{body()}</span></div>;
}

// The third way a hole arrives, and the negative control for the `<noscript>`
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
// 2.0 only. The 1.x producer does not retract this subtree at all -- it fails
// reconciliation and the file is rejected (`semantic trace has unresolved
// execution sites`), the same shape of producer gap as the `textContent` arm
// above. Recorded in docs/precision-backlog.md rather than pinned, because a
// fixture there would pin an exit code.
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
      <RetractedTextContentShadowedChild />
      <RetractedInertNoscriptChild />
      <TrackedChildStaysCertified />
      <ReadOutsideJsxStaysAViolation />
    </div>
  );
}
