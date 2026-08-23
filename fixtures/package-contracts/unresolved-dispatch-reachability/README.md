# An obligation in a private helper belongs to the exports that reach it

`Panel` is not part of the entrypoint's public surface: `index.js` imports it
and never re-exports it. The unresolved `.getThing` dispatch lives inside
`Panel`, so no enclosing function of the obligation is an export and the
lexical ladder has no answer.

The call graph does. `Reaches` calls `Panel`; `Isolated` does not. Only
`Reaches` is marked, and only in the two domains the dispatch invalidates.
Before this, both exports collapsed into one summary with all five domains
unknown -- `Isolated`, which cannot reach the obligation by any path, published
as entirely uncertain.

The `because` note on each `unknown-sentinel` item of
`<contract>.review.json` records `reachability` as the rung that answered, and
names the file and byte range in `panel.js` the obligation sits at, which is
not otherwise recoverable from the contract.

## Why the enumeration is trusted here

`Panel`'s only references are its declaration, its `export` in `panel.js`, its
`import` in `index.js`, and the resolved call in `Reaches`. Nothing takes it as
a value, so the caller set the graph enumerated is the whole entry set. A
function that escaped into a callee the analysis cannot resolve -- passed as an
argument, returned to the caller, handed over as a prop value -- makes the
enumeration incomplete, and attribution falls back to marking every export
rather than trusting a partial answer.

That last sentence was false when it was written. The escape test accepted any
reference inside an `ExportFact.span`, and an `ExportNamedDeclaration`'s span
covers the declaration's whole body -- so `apply(Panel, ...)` and
`return Panel` read as export surface, the escape was never seen, and the
partial answer was trusted. `fixtures/package-contracts/escaping-private-helper`
is the fixture that makes the sentence true: one entrypoint per escape shape,
each widening to every export, beside a `./called` control that keeps this
fixture's exact behavior.

A rendered tag is *not* on that list, though it was, for a different reason: the
call graph enumerated only call expressions, so the tag named no call site. It
does now (`all_function_call_sites`), because rendering a component invokes it —
in both spellings, `<Panel/>` and `<Panel></Panel>`, since the closing tag's
name span rides on the same edge. `escaping-private-helper`'s `./rendered`,
`./closed`, `./children`, `./shadowed`, and `./builtin` arms pin the exact
enumeration; `./member-tag` and `./member-tag-children` pin the dotted tag,
which still fails closed in both spellings because the edge's callee is the
whole dotted name while the reference the escape test walks is the property
inside it.

`Reaches` is a function declaration. The arrow form of the same shape --
`export const Reaches = props => Panel(...)` -- has no `FunctionFact.name`, was
unnameable at every rung, and was published as certified;
`fixtures/package-contracts/arrow-export-attribution` pins it.

`Reaches` passes a fresh object rather than forwarding `props` itself, so the
fixture pins the reachability rung alone; forwarding the parameter also raises
an unrelated `UnknownCallbackExecution` obligation on `Reaches`.
