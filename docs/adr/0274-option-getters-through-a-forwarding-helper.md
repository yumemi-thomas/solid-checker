# ADR 0274: Option getters through a forwarding helper

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `solid-reactive-ir/src/property_gets.rs`: the helper forwarding census and
    the getter guarantee;
  - `contract_semantics::enumeration_get_is_guaranteed`: the guarantee rule;
  - `solid-facts` `AstFacts::straight_line_constructs` (AST facts schema 58).
- Extends ADR 0225. No contract vocabulary, document or receipt changes.

## Context

ADR 0225 proved a package value copy (`get-enumerable-string-values`) reads
the caller's option getters at the call. It left two gaps that kept every
real corpus call open:

- The apps construct the `Virtualizer` through their own helper module
  (probus-hk `createCustomVirtualizer(options)`), and only a literal passed
  directly to the package call bound.
- Only the first getter was guaranteed: a later getter runs only if earlier
  ones complete.

## Decision

1. **A forwarding helper performs the copy at its own call.** A call of a
   local helper is treated as the package call for one parameter when all of
   these hold:
   - the helper resolves exactly, is current, and is neither async nor a
     generator;
   - its body calls the package directly and unconditionally (straight-line
     flow, no nested function, no optional chain);
   - it passes that parameter itself as the enumerated argument;
   - the parameter is a plain, unwritten, default-less identifier;
   - nothing references the parameter before that call.

   The caller's argument must then be the same exact receiver ADR 0225
   requires, and the getters run with the caller's call role. `new`
   expressions get their own straight-line table, so the existing
   `straight_line_calls` consumers keep their meaning.
2. **Every getter of a hazard-free prefix is guaranteed.** The owner's
   statically-present-code premise (2026-10-13) puts an earlier getter's
   abrupt completion out of scope. The same premise puts reads in a reached
   getter body inside it, not only its first call. A getter that may reach
   the receiver mutates the enumeration. The receiver is a fresh unaliased
   literal, so `this` is its only route, and any `this` token in a getter
   leaves every later getter possible rather than guaranteed.
3. **Result reuse.** A file holding such a helper is call-role syntax, so
   editing it invalidates its callers' retained results.

## Consequences

- rc.13 corpus (browser host): 568 → 573 violations, 0 removed. All five are
  at probus-hk `VirtualRows.tsx:81`, `createCustomVirtualizer({...})`: four
  `scroller()` getter reads (`getScrollElement`, `observeElementRect`,
  `observeElementOffset`, `scrollToFn`) and `margin()` in `scrollMargin`.
- The getter declarations keep their callback-consumer obligation, as in the
  direct ADR 0225 case.
- Not yet:
  - ai-memory-ui, which merges options through Solid `merge` before
    constructing;
  - table-core 9.2.4, which has no generated identity;
  - virtual-core 3.17.10/3.17.11.

## Evidence

- Unit tests: `a_helper_forwards_only_an_untouched_parameter_on_its_straight_line`
  and `this_is_matched_only_as_a_token`.
- Twin `probus-virtual-helper-forwarded-later-getter` in
  `fixtures/app-patterns-misuse/corpus-twins.json`. Natively, the misuse twin
  has `rows` and `margin` violations at the helper call and the correct twin
  has none, and both pass `tsc`.
- Runtime, using probus-hk's installed rc.13 development runtime and
  virtual-core 3.17.8 (`rust/target/research/provenance-2/getter-runtime/`):
  - the helper misuse raises `STRICT_READ_UNTRACKED` twice (count, then the
    later scrollMargin getter);
  - the `untrack` form raises none;
  - the probus-shaped conditional getter read raises one.
