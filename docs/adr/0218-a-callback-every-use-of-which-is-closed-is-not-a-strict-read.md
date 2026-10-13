# ADR 0218: A callback every use of which is closed is not a strict read

- Status: accepted and implemented (2026-10-06).
- Owners:
  - `ReadConsumerSummaries` (`solid-reactive-ir/src/project_consumer.rs`, a
    child of `execution_role.rs`);
  - `ReactiveRead::project_consumer_non_strict` and its strict-read
    projection (`projection.rs`);
  - `Dialect::callback_may_open_strict_window` (`solid-dialect`);
  - `runtime_semantics::dispatches_through_receiver_then`;
  - `AstFacts::tagged_template_tags` (`solid-facts/src/ast/mod.rs`).
- Relation: narrows the strict-read rule (`SC1001`) for callbacks that a
  component body hands to project functions and components.

## Context

A reactive read inside a callback that a component body passes to a project
function stayed uncertifiable unless the checker could prove when the callback
runs. Most such helpers do the safe thing:

```tsx
function useDropdownPosition(layout: () => Layout | null) {
  createEffect(layout, (layout) => { … });   // layout is the effect's compute
}
const pos = useDropdownPosition(() => (open() ? { … } : null));
```

The read runs only inside a tracked compute, so Solid 2.0.0-rc.13 raises no
`STRICT_READ_UNTRACKED`. The same holds for a callback prop read inside a
`createMemo`, and for a callback that runs under unlabelled `untrack` or from
a fresh host queue.

## Decision

1. **The subject is exact.** It is a function declaration, a parameter index
   and, optionally, one static property of that parameter. The callback is an
   arrow function passed as a direct argument, as a field of an exact object
   literal passed as an argument, or as a static attribute of an exact project
   component. An arrow has no `this` of its own. Any other function is called
   as a member of the object holding it, and can hand that receiver to code
   that calls it again later (`replay = () => this.read()`).
2. **Every runtime reference to the parameter is classified.** The local
   syntax census and the Type Facts reference index are unioned, and any
   reference outside the function refuses the proof. A use is closed when it
   is:
   - a dialect-proven tracked compute slot;
   - exact unlabelled `untrack`;
   - a compiler-tracked JSX expression;
   - an audited fresh-stack scheduler from the host timing table;
   - an exact forward to another closed subject.

   Each enclosing function carrier is closed as well, so an `untrack` inside
   a returned closure is still an escape. Storing, returning or capturing the
   callback is open. So are:
   - aliasing it;
   - handing it to an unknown package, a member target or a DOM listener;
   - a default value, `arguments`, or a cycle;
   - no use at all.
3. **A field's siblings must not reach it.** For a field subject (`o.key`):
   - **Invoked siblings.** Calling another field of the same object, or
     using it as a template tag, runs it with the whole object as `this`, as
     in `o.invoke()` calling `this.key()`. So each such sibling must be an
     arrow function at every site that builds the object, written there or
     named by an exact `const`. An absent, named or method sibling refuses
     the proof: an absent one is inherited (`valueOf` returns the object).
   - **Read siblings.** A sibling that is only read must be present at the
     site. An absent one is inherited, and reading it runs any getter project
     code installed on a prototype, under whatever spelling.
   - **Dereferences and writes.** A dereferenced sibling (`o.x.y`) refuses
     the proof, as does any write or delete of a field after construction
     (`o.invoke = function () { … }`, `o.__proto__ = …`).
   - **Prototypes.** An argument literal with a `__proto__` entry, and an
     element with a `__proto__` attribute, are refused, because each installs
     a prototype.

   The sites are the object-literal argument, and the JSX element at the
   root and at each forward. Template tags are a new syntax fact (facts
   schema 49).
4. **Strict windows are never inherited.**
   - A labelled `untrack(fn, label)` sets `strictRead` to the label
     (`@solidjs/signals` `untrack`, rc.13), so it refuses the proof wherever
     it sits, a memo or a queue included.
   - Every slot the dialect reports untracked reads for resets to strict.
   - A slot the dialect says may open a window of its own from an option is
     refused before its tracked reset (`callback_may_open_strict_window`).
     For Solid 2 these are `mapArray` and `repeat` with an options argument,
     which wrap the map function in `setStrictRead(options.name)`.
5. **A component entry opens its own strict window.** A JSX forward into a
   component prop maps the child's body-time uses to strict, not to the
   carrier the element is built in. The runtime enters every component
   through `untrack(() => Comp(props), label)`.
6. **`Promise.catch` and `finally` are not fresh-stack here.** Both call
   their receiver's `then` (ECMAScript 27.2.5.1, 27.2.5.3). A subclass that
   overrides `then` can run the handler synchronously, and the inherited
   method still resolves to the standard declaration.
7. **The proof clears the strict-read finding only.** It is a separate field
   of `ReactiveRead`, skipped by serialization, recomputed after both read
   tables merge, and read only by strict-read projection. Write, ownership,
   async, dispatch and package-contract obligations keep their roles.

## Consequences

- A callback handed to a hook that only computes, untracks or queues it is no
  longer an uncertifiable strict read.
- Kept for later, all still uncertifiable:
  - a callback stored in an object field and read back;
  - a callback passed to a package helper without a contract, such as a
    query key;
  - a DOM listener whose dispatch may be synchronous;
  - a callback whose consumer is async or has a defaulted parameter;
  - a whole props object handed on;
  - an element with a spread;
  - `mapArray` with options;
  - `Promise.catch` and `finally`.
- A first version also proved a forwarding chain of expression-bodied
  functions ending in a body-time call (`const forward = (r) => inline(r)`)
  as a violation. It moved no corpus finding, and a `const` callee may be
  uninitialized when the chain runs, so it was dropped.
- **Open, not caused by this change:**
  - A labelled `untrack` written directly in a component body
    (`untrack(() => count(), "label")`) warns at runtime, and the checker
    reports nothing there (fixture case `DirectLabelledUntrack`).
  - The ADR 0210 leaf-scope host timing still treats `Promise.catch` and
    `finally` as fresh-stack.

  See docs/precision-backlog.md.

## Evidence

- **Research proposal** `rust/target/research/volume/hooks/` (a Codex agent).
- **Four adversarial review rounds** by Codex agents
  (`rust/target/research/review-hooks.md`, `review-hooks-r2.md`,
  `review-hooks-r3.md`, `review-r4.md`). Each counterexample is now a fixture case, and each
  round's fix only narrowed the proof. The rounds found:
  1. a sibling invoked through `this`, labelled `untrack`, and a throwing
     default in the inline chain;
  2. a sibling overwritten after construction, `__proto__` writes and
     attributes, an extended `Object.prototype`, labelled `untrack` under a
     memo, a component entered inside a memo, and an argument in its temporal
     dead zone;
  3. an absent sibling read through an installed getter, a named
     `mapArray`, `Promise.catch` through an overridden `then`, and a callee
     in its temporal dead zone;
  4. a getter installed through a computed key on an aliased `Object`, and
     a `function` subject handing its receiver to a sibling getter.

  A fifth round (`review-r5.md`) found nothing further in this change.

  A first version of the sibling rule refused every invoked sibling, which
  gave back 16 of 24 corpus removals; most invoke an arrow sibling such as
  `estimateSize: () => 84`.
- **Fixture** `project-callback-uses`, which type-checks clean against the
  published `solid-js@2.0.0-rc.13` and `@solidjs/web@2.0.0-rc.13`
  declarations: 12 clean cases plus a clean object field and two clean
  sibling cases, 3 violations, and the rest uncertifiable.
- **Coverage:** besides the new snapshots, four reads that were
  uncertifiable become clean:
  - `forwarded-tracked-compute`'s `UsesUntracked` and `UsesForwarded`;
  - `forwarded-event-prop`'s `UsesKeeps`, a prop invoked from a timer;
  - `render-time-reads`' `RenderLive`, a prop called only in JSX.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-aa-browser.json`: uncertifiable 3,397 to 3,384, 13 strict reads
  removed, none added; violations unchanged at 195. The removed sites checked
  by hand are:
  - `useDropdownPosition`'s effect compute;
  - `createDeckQuery`'s `untrack` inside an effect apply;
  - `WorkspaceSnapAssistMasterGrid`'s `createMemo(() => props.getHoverPick())`.
- **Runtime ledgers** (release binary, Chrome):
  - the base misuse ledger is unchanged: 31 of 33 detected at runtime, 29
    reported as violations;
  - the 18 corpus twins are unchanged: all detected at runtime, and
    statically 4 violations, 11 uncertifiable and 3 silent.
