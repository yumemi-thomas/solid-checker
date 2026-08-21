# realistic-topology

A project shaped like a project, because the corpus's blind spot was shape, not
coverage.

Three real defects were found in this engine during one week's work, and none
of them moved a single fixture across 76 projects. All three were found by
writing a scratch project by hand. The reason is visible in what the fixtures
looked like: the `interprocedural` fixture was two files and seventeen lines
with one shape, and the component fixtures are single files whose exported
components have no callers at all. They test the shapes the authors thought of.
Real projects have components in their own files rendered by other components,
helpers called from component bodies, and a module-scope source read across
files — and it is exactly those facts, the ones that only exist because more
than one file is in the program, that decide most findings.

`.solid-checker/runtime.json` selects `programBoundary: "closed"`, because that
is what an application is. The result is the point:

| Site | Outcome | Decided by |
| --- | --- | --- |
| `Card` reads `props.title` | violation | a caller in *another file* passes a memo |
| `Badge` destructures `label` | violation | the same, through a parameter destructure |
| `readCountNow()` in a render scope | violation | a cross-file summary: the read is in the helper's own body |
| `Plaque` reads `props.note` | **silent** | every caller passes a static value, and the program is closed |
| `watchCount()` in a render scope | **silent** | the read is sealed inside a tracked callback, so the caller performs none |
| the effect inside `watchCount` | **silent** | called from a component body, so the owner is proven |

Six sites, three findings, all of them proven, and no obligations. That is what
a well-analyzed application should look like, and it is only reachable because
the topology supplies the facts.

The two silent rows carry the regressions worth guarding. `watchCount` is the
false-positive shape: its only read lives inside `createEffect`, and until the
interprocedural summary learned to respect a callback's declared execution,
calling it charged the caller with a *proven violation* for a read that never
happens. `Plaque` is the over-conservatism shape: a complete static caller set
certifies, and the same file with the selector dropped turns both rows plus the
owner into obligations — which is how to check that the topology, and not a
blanket amnesty, is doing the work.

The open-world behaviour these rows would otherwise pin is covered where it
belongs: `program-boundary-closed` compares one set of files under both
boundaries, and the `engine/eslint-reactivity-*` and `eslint-plugin-corpus*`
fixtures keep the default deliberately.
