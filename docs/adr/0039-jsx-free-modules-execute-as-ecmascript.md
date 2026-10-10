# ADR 0039: A `.jsx` module with no JSX executes as ECMAScript

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: the probe harness (`probe_harness.rs`,
  `packages/cli/scripts/contract-probe-worker.mjs`)
- Relation: narrows the `vetoThrew` withholding class of ADR 0036 for one
  measured shape. Worker protocol v5 → v6; the lever-B slice of
  `docs/package-contract-v2/accuracy-roadmap.md`, which this ADR also corrects.

## Context

A closure candidate whose mandatory veto does not complete is withheld by name
(ADR 0036 § 2). On the 2026-09-06 corpus 62 candidates across four rows were
withheld as `vetoThrew`, and the roadmap's lever B assumed the cause: a `.jsx`
runtime target that the pinned interpreter cannot execute, to be closed by
deriving compiled bytes with the pinned Solid compiler and recording the
compiler and its options as a premise.

The measured reasons say otherwise. Every one of the 61 `.jsx` failures reads

```
TypeError: Unknown file extension ".jsx" for …/node_modules/@corvu/utils/dist/…
```

and the modules behind them are not markup. `@corvu/utils@0.4.2` publishes 23
`.jsx` files under its `solid` condition; **20 of them contain no JSX at
all**. The chunk the failures reach is exactly this:

```js
var isFunction = (value) => typeof value === "function" && value.length > 0;
var dataIf = (condition) => (condition ? "" : void 0);
export { isFunction, isButton, dataIf };
```

A `.jsx` extension is a *bundler convention*, not a syntax. A package that
ships a `solid` condition names its uncompiled sources `.jsx` so a consumer's
Solid JSX transform picks them up, and every such module that declares helpers
rather than markup is already ordinary ECMAScript. Node refuses it for the
extension alone. Compiling it would be answering a question nobody asked: there
is nothing in it to compile.

The 62nd failure is a different shape and is not addressed here — pinned Node
refuses to strip types from a `.ts` file *under `node_modules`*
(`@kobalte/utils`), which is an erasure premise, not an extension one.

## Decision

**A module whose extension is `.jsx` is executed by the veto worker as
ECMAScript, admitted only when the checker's own parse of the authenticated
bytes reports no JSX element and no JSX fragment, and the receipt records the
premise.**

The premise is exactly: *a JSX transform is the identity on a module with no
JSX*. It is not a claim that the `.jsx` and the compiled `.js` sibling agree —
nothing here reads the sibling — and it is not a claim about any compiler.

### What the checker admits

`jsx_free_modules_of` walks every `.jsx` member of an authenticated snapshot —
the plan's own and each authenticated dependency's, because the veto of a graph
node imports that node's package and a parent's veto reaches its dependencies —
and admits a member only when all of these hold:

- the bytes are UTF-8;
- `solid_facts::ast::extract` parses them (the same parser the `creates` census
  reads authenticated bytes with, which enables JSX for a `.jsx` path, so a
  module carrying markup parses *successfully* and is refused on its facts
  rather than on a parse error);
- both `jsx_elements` and `jsx_fragments` are empty.

Each admitted module is recorded as the file it was written to inside the
private tree together with the SHA-256 of the **authenticated** bytes.

### What the worker does

When the session carries `jsxFreeEsm` — it is absent unless something was
admitted, so a package of compiled JavaScript runs exactly the pre-ADR path —
the worker installs one `node:module` load hook before any package or recipe
code runs. For a `.jsx` URL the hook:

- refuses by name every URL the checker did not admit, so a module outside the
  premise cannot reach the interpreter through the hook and be reported as an
  ordinary load failure;
- re-reads the admitted file and refuses unless its digest is the admitted one,
  so a `.jsx` swapped under the private tree between admission and load throws
  rather than executing (the watched-input census is the independent other half
  of that answer);
- refuses CommonJS consumption;
- otherwise returns the bytes as `format: "module"`.

The hook is installed only in the ordinary published-bytes lane. A controlled
execution profile (ADRs 0026–0033) serves its own module set and resolves no
`.jsx`.

### Why this cannot pass as something else

Two independent parsers must agree, which is the same shape as ADR 0030's
derive-and-recompute. The checker admits on its own parse; **the interpreter is
the falsifier**. Every JSX form is a syntax error in ECMAScript, so a module
this walk admitted in error throws inside the worker and the candidate stays
withheld. There is no reading under which a module with markup runs as
something else.

### What the receipt says

One field in the probe-gate root: `jsx-free-esm:<count>:sha256:<digest>`, the
digest taken over the admitted modules' sorted paths and byte digests. Absent
when nothing was admitted, so every receipt of a package that publishes only
compiled JavaScript is unchanged by this ADR, and a receipt that names the
field states the condition its gates ran under. Worker protocol v5 → v6,
because a run's meaning changes: a `.jsx` module may now execute.

## Alternatives considered

- **Compile the `.jsx` with the pinned Solid compiler** (lever B as written).
  Rejected *for this class*: there is nothing to compile, and a receipt over
  compiled bytes would name a compiler and its options as a premise where the
  honest premise is far narrower. It remains the answer for the three
  markup-carrying modules, and for a `solid`-condition entry that really is
  JSX — with the 1.x half still needing the independent Babel reproduction the
  roadmap describes, because a receipt over our port's output is a receipt
  about the port.
- **Rename the admitted modules to `.js` and rewrite their specifiers**, an
  ADR-0030-style derived graph. Rejected: it changes the bytes that run, so the
  premise would grow a rewriter nobody needs — the load hook leaves every byte
  the interpreter executes exactly as published.
- **Let Node decide alone** — install the hook for every `.jsx` and rely on the
  syntax error. Rejected: it would be sound, but the checker would then hold a
  claim under a premise it never stated, and the receipt could not name it.
- **Admit the compiled `.js` sibling instead.** Rejected: the artifact case
  selected the `solid` condition, and the certified bytes are the `.jsx`.
  Executing the sibling would observe a different artifact case.

## What still refuses

- **A `.jsx` module that carries markup** — `components/Fragment.jsx`,
  `components/FloatingArrow.jsx`, and one chunk in `@corvu/utils`. Its veto
  keeps withholding, now by the hook's own name rather than by Node's
  extension error.
- **A `.ts` module under `node_modules`**, which pinned Node refuses to strip
  (`@kobalte/utils`, one candidate). An erasure premise, not this one.
- **A `.jsx` module that is not UTF-8 or does not parse.**
- Everything the pre-ADR path refused: nothing here widens what a veto observes
  or what a contradiction means.

## Consequences

- Worker protocol v5 → v6. The worker is a member of the harness source
  manifest, so its digest and `packages/cli/probe-harness.buildinfo` move with
  this change; every build through the Makefile recomputes both, and a checker
  built against the old stamp refuses the gate rather than running the new
  worker.
- Measured on the 2026-09-06 corpus: withheld candidates 866 → 805,
  `vetoThrew` 62 → 1 — the whole `.jsx` class closed, and the one left is the
  `.ts`-under-`node_modules` erasure case above. 61 candidates that had no
  verdict now have one. Statuses unchanged, 368 certified / 30 refused. The
  three markup-carrying modules in the same packages are what exercises the
  hook's refusal path on every one of these runs.
- Tests: `only_a_jsx_member_that_parses_without_jsx_is_admitted` and
  `the_jsx_free_premise_field_names_the_admitted_set_and_is_absent_without_one`
  pin the admission and the receipt field; the corpus rows above are the
  end-to-end evidence, including the hook's refusal path, which the
  markup-carrying modules in the same packages exercise.
- `docs/package-contract-v2/accuracy-roadmap.md` lever B is corrected: its
  measured class was mostly an extension, not a compilation.
