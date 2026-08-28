# A private helper that escapes as a value breaks the caller enumeration

The call graph's answer is *fail closed or exact*: either every way of entering
a function is one of the call sites it enumerated, or the enumeration is
incomplete and emission must mark every export. A function referenced anywhere
other than its declaration, one of those call sites, or the module surface that
forwards it has escaped as a value — something the graph did not model can
invoke it.

The escape test used to accept any reference inside an `ExportFact.span`. For an
`ExportNamedDeclaration` that span covers the **whole declaration, body
included**, so every reference written inside an exported function read as
"export surface". `apply(Panel, ...)` and `return Panel` are all such
references: each one is a real escape, each one was accounted for, and the
enumeration was then trusted while being wrong. Only the export that happened
to *call* the helper was marked; the export that leaked it — and every export
beside it — was published as certified.

A rendered tag used to read as an escape too, for a different reason: the call
graph enumerated only call *expressions*, so `<PanelView/>` named no call site
and the reference was unaccounted for. Rendering a component invokes it, so the
tag is a call site; `all_function_call_sites` now emits an edge for the tag name
span whenever it resolves to one exact project function, and the escape test
accepts that reference for the only reason it accepts any other — the edge is in
the graph. Nothing accepts a tag *because* it is a tag: a tag that resolves to
nothing emits no edge and is still an escape.

A tag with a closing tag writes the component's name **twice**, and TypeScript
reports both occurrences as references to the same symbol. The edge alone
therefore accounted for `<Panel/>` and not `<Panel></Panel>`, which is the
dominant real-world spelling — the paired form regressed to marking every
export. `solid-facts` records the closing tag's name span
(`JsxElementFact::closing_name`), the edge carries it as a second *reference*
rather than a second call site (one render is one invocation), and the escape
test accounts for both spans. The closing span can never account for anything on
its own: it is stored on the edge that the *opening* tag's resolution created,
so a tag whose opening name resolves to nothing has no edge to carry it.

One entrypoint per claim, because a single entrypoint could not distinguish
them: every escape widens to *all* exports of its entrypoint, so two escapes in
one export map would mask each other. Each arm gets its own private helper for
the same reason — an escape is decided over every reference of that symbol in
the whole package, so one arm leaking a helper the next arm renders would decide
both arms.

Exact enumeration — the escaped helper's caller is known, only that caller is
marked, and `Isolated` stays certified:

- `./called` — the control. `Direct` calls `Panel`; nothing takes it as a value.
  This is the export that a widen-everything regression would break.
- `./rendered` — `App` renders `<PanelView/>`, self-closing. The tag is the call
  site, so this arm is shape-identical to `./called`.
- `./closed` — `App` renders `<ClosedView></ClosedView>`. The only textual
  difference from `./rendered` is the closing tag, and that is the point: the
  arm is shape-identical to `./rendered`, and was `fallback-all` until the
  closing name span was recorded and accounted for.
- `./children` — `App` renders `<ExprView>{props.label}</ExprView>`: the paired
  form with a real child, which is how a component with content is actually
  written. Same answer as `./closed`; the child changes nothing, because what
  the escape test needed was the closing tag's name.
- `./shadowed` — the rendered helper is named `Show`, which is also the spelling
  of a Solid 1.x control-flow built-in *in the dialect vocabulary this project
  analyzes with*. Resolution is by symbol, not by spelling: `Show` here is the
  project function `show.jsx` declares, it gets the edge, and the dialect entry
  of the same name never enters the decision. (The arm does not import
  `solid-js`; it does not need to. A name-text rule would misfire on the
  vocabulary alone.)
- `./builtin` — `App` renders a real dialect built-in, `<For>` from `solid-js`,
  and renders `<ListedView/>` inside the child callback it hands `For`. `For` is
  not a project function, so it contributes no edge and escapes nothing; the tag
  nested in the callback still resolves, and its caller is the outermost
  declaration, `App`.

No enumeration needed — nothing renders the helper at all:

- `./intrinsic` — `App` renders `<div/>`, and `lower-view.jsx` exports a project
  function *named* `div` that the entrypoint imports and never uses. TypeScript
  binds a lowercase tag name as an intrinsic element name and never against the
  value scope, so the tag resolves to no project function, emits no edge, and
  the obligation inside `div` reaches no export: both exports stay certified.
  This is the arm a name-matched edge would break, and the only arm here whose
  claim is a certified negative — it is correct because `<div/>` genuinely never
  calls it. Note the claim is about *binding*, not about intrinsic-element
  typing: see "Faithfulness" below for why no `JSX.IntrinsicElements` is in
  scope during contract generation, and why that does not weaken the arm.

Incomplete enumeration — the helper escaped as a value, so every export of the
entrypoint is marked:

- `./argument` — `Escaped` hands `Panel` to `apply`, which invokes it.
- `./returned` — `makePanel` returns `Panel` to the caller.
- `./prop-value` — `Held` renders `<Wrap child={HeldView}/>`. `HeldView` is an
  attribute value, not a tag: `Wrap` decides whether and when to invoke it, so
  it is exactly the escape the tag edge must not swallow.
- `./member-tag` — `App` renders `<dotted.DottedView/>` through a namespace
  import. The tag *does* resolve: TypeScript reports the component's symbol at
  the whole `dotted.DottedView` name span, so the edge is emitted with that
  whole span as its callee. What fails closed is the span mismatch — the
  reference the escape test walks is the `DottedView` property *inside* the
  name, and the test is byte-exact span membership, not containment. Making a
  dotted tag exact means having the edge's callee and the walked reference name
  the same span; it is future precision work, not a correctness gap.
- `./member-tag-children` — the same dotted tag in the paired form,
  `<paired.PairedView></paired.PairedView>`. It is the negative that the closing
  span work needs: the closing tag is now accounted for wherever the opening tag
  resolved, and this arm proves that acceptance did not become syntactic. Both
  of the dotted tag's spans still mismatch the property reference, so the arm
  stays `fallback-all` — `sameAs ./member-tag`, which is the pin. (A
  closing-tag-only mismatch cannot be written: the two tags spell the same name,
  so a dotted opening tag always has a dotted closing tag.)

Widening to every export here is imprecise, not wrong: nothing in the package
proves the escaped helper is unreachable from `Isolated`'s caller, and the
opposite error publishes a certified negative about an export whose behavior
depends on an unresolved obligation.

`./rendered`, `./closed`, `./children`, `./shadowed`, and `./builtin` produce
identical export maps, so the contract records the rest as `sameAs` the first
one alphabetically — `./builtin`. That dedup is the pin: an arm that regressed to
fallback-all would stop being `sameAs` anything. `./member-tag-children` is
`sameAs ./member-tag` for the mirror-image reason.

`Wrap` in `./prop-value` and the intrinsic `div` are the two spellings that make
the resolution requirement visible: a tag is a call site only when it resolves
to exactly one project function, never because of how it is written.

The declarations are exact for this fixture package. `channel.js`, `panel.js`,
and each arm's private view component deliberately have no sibling `.d.ts`:
with one, every importer binds to the declaration file, the caller edges vanish,
and the enumeration widens to every export for a reason that has nothing to do
with an escape — which would make most of these entrypoints pass for the wrong
reason. `declaration-sibling-reach` is where that shape is pinned.

## Faithfulness of the `solid-js` stub

`node_modules/solid-js/` selects the Solid 1.x dialect (version `1.9.14`) and
supplies the typings `./builtin` and `./shadowed` make claims about. It used to
carry no typings at all — a `.js` module exporting only `createSignal` — so
`import { For } from "solid-js"` was an untyped `any` and the two arms' *stated
reasons* were untested even though their outcomes were right.

`index.d.ts` is transcribed from solid-js@1.9.14. `For` and both `Show`
overloads (with `RequiredParameter`), `Accessor`, `Setter`, `Signal`,
`createSignal`, and `JSX.Element` are byte-faithful to the published
declarations (`types/render/flow.d.ts`, `types/reactive/signal.d.ts`,
`types/jsx.d.ts`). Three things are deliberate *subsets*, never supersets:
`SignalOptions` inlines the two members its real inheritance chain contributes,
`JSX.HTMLAttributes` carries three of the real interface's members, and
`JSX.IntrinsicElements` lists the three tags the fixture writes. A stub narrower
than the package cannot manufacture a finding; a looser one can, which is why
nothing here is widened.

`JSX` is an `export namespace` inside the module, exactly as solid-js declares
it — the published package contributes no *global* `JSX`, which is why a real
Solid project sets `jsxImportSource: "solid-js"`. The temporary-v2 package
producer writes an isolated analysis config with `jsx: "preserve"` and no
`jsxImportSource`, so during generation no `JSX` namespace is in scope at all.
Two consequences, both recorded rather than papered over:

- `./intrinsic`'s claim is about TypeScript's *binding* rule — a lowercase tag
  name is an intrinsic element name and is never looked up in the value scope —
  which holds whether or not `JSX.IntrinsicElements` exists. The arm does not
  depend on intrinsic-element typing, and would give the same answer if it did:
  verified against the published typings under `jsxImportSource: "solid-js"`.
- `builtin.jsx` reports `TS2741` (`For`'s required `children` is missing) under
  the generator's tsconfig, because without `JSX.ElementChildrenAttribute` in
  scope TypeScript cannot map a JSX child onto the `children` prop. That
  diagnostic is *identical with the real published package installed*, so it is
  a property of the generator's tsconfig and not of this stub; see
  docs/precision-backlog.md. It changes no claim in the generated contract.

`tsc --noEmit` results, over every `.js`/`.jsx` implementation file as explicit
roots (the arms' `.d.ts` siblings shadow them under `include`, which is how this
check was silently vacuous before):

| typings | tsconfig | result |
| --- | --- | --- |
| this stub | generator's (no `jsxImportSource`) | `builtin.jsx` TS2741, nothing else |
| real solid-js@1.9.14 | generator's (no `jsxImportSource`) | the same TS2741, nothing else |
| real solid-js@1.9.14 | `jsxImportSource: "solid-js"` | clean |

The third row is the one the absolute rule cares about: against the real
published typings in the configuration a Solid project actually uses, every arm
type-checks, so no finding this fixture pins duplicates a `tsc` diagnostic. That
row is also why `./intrinsic` renders a bare `<div/>`: it used to write
`<div client={props.client}/>`, which is `TS2322` against the published
`HTMLAttributes<HTMLDivElement>`, and the arm never needed the attribute.
