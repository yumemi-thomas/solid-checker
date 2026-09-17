// The **refusing** half of the `reads` implementation census tracer.
//
// A separate entrypoint, and that is the point. The premise the census cannot
// obtain — "no accessor installed at run time reaches this read" — is a fact
// about a *closure*, not about an export: TypeScript types a `Proxy` as its
// target, so a read through one records no form for any census to refuse
// (`docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
// § 6-§ 9). The syntactic hazard is the only place it is still visible, and it
// can only be attributed to the file that carries it.
//
// So every export here refuses `reads`, including ones that never touch the
// proxy. That is the design, not a limitation of this fixture: attributing the
// hazard to individual exports would need dataflow from the `new Proxy`
// expression to each read's receiver, which nothing in this pipeline has.

let observed = 0;

// A value this module built whose reads run *this module's* code. Nothing in
// the syntax distinguishes it from an ordinary object literal, which is the
// point.
const ownProxy = new Proxy(
  { value: 1 },
  {
    get(target, key) {
      observed += 1;
      return target[key];
    }
  }
);

// --- `reads: []` must refuse for every export below. ---

// A receiver this program built by *calling* something. Its subject root is a
// call result, which `census_form_disposition` does not clear, so the domain
// refuses rather than closing over a trap it cannot see.
export function readsOwnProxy() {
  return ownProxy.value;
}

export function readsOwnProxyElement(key) {
  return ownProxy[key];
}

// --- Probe observability, not a subject. ---
// How many times `ownProxy`'s trap ran. A hand-authored recipe can watch this
// module's own source exactly *because* its author knows which source the
// module owns — the property a generic synthesized veto cannot have.
export function observedReads() {
  return observed;
}
