# reactive-dispatch-unresolved

`SC9012` · **warning** · uncertifiable

A type-correct call can reach runtime implementations with different or
unknown reactivity or ownership behavior, or an exact synchronous callback
position receives a body the checker cannot inspect. The call therefore cannot
be certified as safe or reported as a proven violation.

## What it does

Reports a proof obligation when exact runtime dispatch is required by the
reactivity analysis but cannot be established. Covered forms include:

- a helper that invokes `argument.method()` when the argument is a conditional
  or union of objects whose method summaries differ;
- a computed call such as `handlers[index]()` whose runtime target is not an
  exact symbol; and
- an exported helper that directly invokes a member supplied through a
  parameter outside a proven tracked JSX region, because callers outside the
  analyzed project can supply unseen behavior.
- a leaf-owner API receiving a callback through a conditional, aliased,
  package, or otherwise opaque factory/wrapper, or an exact callback whose
  synchronous helper chain reaches an unresolved call. A specific
  cleanup/flush/primitive violation is not proven, but the leaf callback is
  not certifiably free of all three either; closed local returns of one
  function literal or the exact callback parameter are followed instead; and
- an exact built-in synchronous callback position after tracking has ended,
  such as `Array.prototype.filter` after `await`, when the callback body is
  hidden behind a wrapper or is async; and
- an exported structured return whose shorthand value depends on an ambiguous,
  bare/path-mapped, or global binding that cannot be joined exactly; and
- a callback that runs whenever the call's **returned object is read**, which
  is rc.9's `omit(props, predicate)`. See below.

### `omit` predicates (Solid 2.0.0-rc.9)

`@solidjs/signals@2.0.0-rc.9` accepts `omit(props, hidden)` with a single
function argument (`keys.length === 1 && typeof keys[0] === "function"`,
`dist/dev.js:4380`). Where `Proxy` exists, `omit` stores the predicate in the
view it returns, and the view's traps call it on every property get, `in` test
and key enumeration, and on every merge or spread of the view
(`dist/dev-shared.js:273`, `dist/dev.js:3495-3498`, `:4178-4241`). Those calls
run in the reading computation's tracking scope and under its owner. Without
`Proxy`, `omit` calls it once per property during the call (`:4408-4427`).

The call site does not decide where the view is read, so code inside the
predicate is never reported as a violation. It is not placed in the component
body either. A predicate is certified only when it is proven inert, and
otherwise it gets this finding at the predicate argument, with one of three
reasons in `analysisContext`:

| reason | the predicate |
| --- | --- |
| `result-access-callback-reactive-operation` | reads or writes reactive state: a recorded read, write, action or async read, or a reference to a signal, setter, action, store or props binding |
| `result-access-callback-opaque-call` | calls something that is not a resolved standard-library function, such as a project helper, a package export or an unresolved callee |
| `result-access-callback-body-unresolved` | has no inspectable body at this call: an import, a call result, a member, a `let` |

Inert means the literal written at the position, or a `function` or `const`
arrow in the same file that the argument names exactly, contains none of the
above. `(key) => key === "a"` and
`(key) => typeof key === "string" && key.startsWith("_")` are inert. A value
proven not to be a function, like `omit(props, "a")`, is a key list and is
not a predicate.

The same holds one project wrapper away. If
`function hideBy(props, hidden) { return omit(props, hidden); }` is called in
the project, the argument at `hidden`'s position is judged at that call. The
wrapper's own `omit` claims nothing, because the body is its callers'. An
exported wrapper still keeps an open callback for callers outside the project
([package-contract-incomplete](package-contract-incomplete.md)).

Not followed: a predicate forwarded through two or more wrappers, through a
destructured or renamed parameter, or through a spread argument. Code in such
a predicate takes the role of the code around it.

rc.3's `omit` never invokes an argument, and rc.3's typings reject a function
key (TS2345). Under the audited vocabulary, and under a release analyzed with
it, the second argument is a key list and this section does not apply.

Finite candidate sets are not automatically uncertain. If every exact
candidate is present and has the same reactive-read summary, the checker uses
that common summary. If one candidate is missing or the summaries differ, it
reports SC9012 instead of silently choosing a candidate or dropping the call.

SC9012 is shared with Solid 1.x as
[v1/reactive-dispatch-unresolved](v1/reactive-dispatch-unresolved.md). Its
warning severity means that no runtime defect has been proven; its
`uncertifiable` kind still fails `--certify`.

## TypeScript boundary

The rule requires a call that TypeScript facts mark valid. An invalid call is
TypeScript's diagnostic and does not also receive SC9012. The finding asserts
only an unresolved reactivity property that TypeScript types do not express.

## How to fix

Calls inside a compiler-proven tracked JSX expression do not need this
obligation: whichever implementation runs, its reactive reads execute under
that observer. Resolved standard-library methods are likewise excluded.

Narrow the runtime value to one exact implementation, or introduce an adapter
with an analyzed body and one explicit reactive behavior. For a public helper,
avoid invoking structurally supplied methods in a contract that is meant to be
certifiable, or keep the dispatch behind a package boundary with an audited
reactivity contract.

## Related

- [reactive-source-uncaptured](reactive-source-uncaptured.md) — a reactive value crosses an undescribed package call
- [strict-read-untracked](strict-read-untracked.md) — a reactive read is proven to execute untracked
