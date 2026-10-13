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
     flow, no nested function, no optional chain), and the call is the whole
     value of a `const` initializer or of an unconditional `return` written
     directly in the helper. This allowlist excludes operands, branch arms,
     JSX children and fragments, class members and argument positions;
   - it passes that parameter itself as the enumerated argument;
   - the parameter is a plain, unwritten, default-less identifier;
   - nothing references the parameter before that call. A reference inside
     a nested function declaration counts wherever it is written, because a
     hoisted declaration may run first;
   - the helper (its parameters included) never names `arguments`, which
     aliases the parameter without a reference bound to it; no identifier in
     it is spelled with an escape, which could decode to `arguments`; and its
     file holds no direct `eval`, which reaches the parameter with no static
     reference. A callee spelled `eval` under transparent TypeScript wrappers
     (`(eval as typeof eval)(…)`, `eval!(…)`) is direct too;
   - the package call is not inside a class body, JSX element or fragment.
     A field initializer runs at instantiation, and the compiler wraps a
     dynamic JSX prop or child in a getter its consumer runs, so neither
     runs where it is written;
   - neither the helper nor its module carries a directive other than
     `"use strict"`. `"use server"` makes the compiler replace a client
     helper with an RPC reference, so its body does not run at the call.

   The caller's argument must then be the same exact receiver ADR 0225
   requires, and the getters run with the caller's call role. `new`
   expressions get their own straight-line table, so the existing
   `straight_line_calls` consumers keep their meaning.
2. **Every getter of a hazard-free prefix is guaranteed.** The owner's
   statically-present-code premise (2026-10-13) puts an earlier getter's
   abrupt completion out of scope. The same premise puts reads in a reached
   getter body inside it, not only its first call. A getter that may reach
   the receiver mutates the enumeration. The receiver is a fresh unaliased
   literal, so it is reachable only as the getter's receiver: `this`, `super`
   (whose member calls receive the literal), or code `eval` builds. Any such
   token, an escaped identifier, or an `eval` in a getter leaves every later
   getter possible rather than guaranteed. A read inside a class, JSX
   element or fragment written in the getter body is deferred, so it stays
   an obligation.
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

- Review round 1 (rust/target/research/adr-0274/REVIEW-1.md) found three false
  violations: an `arguments[0]` alias, a hoisted mutating declaration, and an
  instance field initializer. Round 2 (REVIEW-2.md) found direct `eval`, an
  escaped `arguments` and `super.valueOf()` in a getter. Round 3 (REVIEW-3.md)
  found direct `eval` behind TypeScript wrappers, in the helper and in a
  getter. Round 4 (REVIEW-4.md) found a construction or read inside a JSX
  prop, which the compiler defers to its consumer. Round 5 (REVIEW-5.md)
  found JSX fragments and a `"use server"` helper. Each now stays
  uncertifiable, and the unit tests pin the syntactic shapes. The helper-side
  rule became an allowlist after round 5.
- Unit tests: `a_helper_forwards_only_an_untouched_parameter_on_its_straight_line`
  and `receiver_routes_are_matched_as_tokens`.
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
