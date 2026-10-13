# ADR 0280: A call's arguments decide its helper's branches

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `solid-facts` `ast::argument_site_standings` (`argument_completion.rs`);
  - `solid-reactive-ir/src/argument_values.rs`;
  - the leaf walk's `Entry::CalledWith` (`cleanup.rs`);
  - the read emission and the by-reference row in `interproc.rs`;
  - `execution_role::parameter_call_standing`;
  - `SemanticLookup::tests_its_parameters`;
  - the `FunctionSource` result dependency (`cache.rs`).
- Recovers the recall ADRs 0275–0279 traded away, at the owner's request
  (2026-10-13). No fact schema, contract or receipt change.

## Context

ADRs 0275–0279 made an effect a helper performs on some of its completions
an obligation, because the branch may depend on the call's arguments. Often
the call decides it:
- `createTimer(fn, 30_000)` takes the `typeof delay === "number"` arm;
- `readWhen(() => 1)` skips it;
- `readFallback()` leaves `value ?? read()` reading;
- `runWhen(true, cb)` calls `cb`.

Those were uncertifiable, and `readWhen(5)`, which reads nothing, was still an
obligation.

## Decision

1. **A call proves parameter values only over an exact mapping.**
   `project_callee_at_parameter` must hold, and no argument may be spread.
   Values come from:
   - the argument's syntax: a boolean, `null`, `undefined`, numeric or
     string literal, or a function, object or array literal;
   - its type, when every constituent is callable;
   - an omitted argument with no default, which is `undefined`.

   A parameter the callee writes, a callee that names `arguments`, and a
   module with a direct `eval` decide nothing.
2. **The walk decides tests and nothing else.** It decides `if` and `?:`
   tests, `&&`/`||` right operands and `??` right operands over `typeof p`,
   truthiness, `== null`, `=== undefined` and their negations. Each site is
   then one of three things:
   - guaranteed: it runs on every completion the caller can continue from;
   - possible;
   - unreachable.

   An earlier statement that may exit makes it possible; one that always
   exits makes it unreachable. A `never` or `asserts` call is a possible
   exit unless it sits in an arm the arguments rule out. Loops, `switch`,
   `try`, logical assignments and optional-chain tails stay possible. In an
   `async` function only the prefix before its first suspension can be
   guaranteed.
3. **Where it applies:**
   - the leaf walk's first helper hop from the leaf callback: an unreachable
     operation is not reported, a guaranteed one is a violation;
   - reads the directly called helper discovered itself, with no hop below
     it: an unreachable one is dropped, a guaranteed one regains its proof;
   - an accessor passed by reference, when the helper calls the parameter on
     some paths only.

   Deeper hops keep the context-free rule.
4. **A decided result depends on the callee's source.** The result records
   a `FunctionSource(path, span)` dependency whose state is the SHA-256 of
   the callee's text. A callee edit then invalidates exactly those retained
   results.
5. **Only functions that test a parameter are evaluated.** A set built once
   from the AST facts skips every other function.

## Consequences

- rc.13 corpus (browser host): 562 → 562 violations, 3,731 → 3,731
  uncertifiable. No corpus call fell in a decided shape the earlier ADRs had
  demoted.
- Cost: about 2% on the two slowest projects in an A/B under the same
  machine load (app-game 26.4 s → 26.9 s, openbot 20.7 s → 21.2 s).
- The vendored `@solid-primitives/timer` calls stay uncertifiable. Its
  shipped body begins with `if (isServer) return;`, which only a host fold
  decides.
- Not yet:
  - deeper hops (`outer(delay)` forwarding to `createTimer`);
  - the inline-arrow callback role;
  - host constants inside the evaluator;
  - argument values carried through local `const` bindings.

## Review round 4

`../solid-checker-rc13-corpus/experiments/review-0280/REVIEW-4.md` found
nine defects, all fixed. Six were silences, where an effect that runs was
dropped as unreachable:
1. An explicit `undefined` passed to a defaulted parameter was taken as
   undefined. A default now replaces any value that may be `undefined`.
2. A parameter redeclared by `var` was taken as unwritten. The IR uses
   `binding_written`, and the evaluator also refuses symbol redeclarations.
3. The string-literal reader accepted text that was not one plain literal.
   Any escape, inner quote of the same kind or line break now refuses.
4. A callable type was trusted without `strictNullChecks`. Types no longer
   prove values; only syntax does.
5. A by-reference callback also reached through another channel
   (`[1].forEach(cb)`, an alias, an argument) was dropped when its direct
   calls were ruled out. Absence now needs every reference to be one of
   those calls.
6. A method replaced by `Object.assign` or `Object.defineProperty` had its
   declared body decided. A method receiver must now be a non-exported
   `const` that every reference only reads a member of. A class must never
   have `.prototype` read or be passed as an argument.

Three were false violations:
7. An optional chain's short-circuit was missed (`sink?.put(read())`). The
   walk now reads chains through the collector's `skippable_chain_parts`.
8. An exit call earlier in the same statement was ignored
   (`use(ensure(ok), read())`).
9. An exit call in a parameter default was ignored.

For 8 and 9, any `never`/`asserts` call in the function before the site
makes it possible, unless the arguments rule that call out.

## Evidence

- Unit tests:
  - `a_number_takes_the_cleanup_arm_and_skips_the_effect`;
  - `review_round_four_refusals`;
  - `truthiness_nullishness_and_refusals`, which also refuses written
    parameters and `arguments`;
  - `literal_truthiness`.
- Fixture `argument-sensitive-reads`:
  - five proven violations;
  - four ruled-out reads with no finding;
  - five uncertifiable results, including a written parameter and a `never`
    guard that runs.
- Fixture `argument-sensitive-leaf`: `onCleanup` only for `30_000`,
  `createEffect` only for `() => 1000`, and both uncertifiable for a union.
- The fixtures of ADRs 0275–0279 now pass `declare const` arguments, so they
  keep testing the rule without argument facts, with their verdicts
  unchanged.
