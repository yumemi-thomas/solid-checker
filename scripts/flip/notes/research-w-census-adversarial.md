# Census adversarial review — `origin/claude/flip-census` at `035286ae0`

Reviewer: agent 2 of 3, read-only. **No checker was run.** Every existing build predates `e40429f5e`, so every "the census certifies" below is **argued from code**, with a confidence level. Chrome and tsc results are real.

- Code read with `git show 035286ae0:<path>`. Copies are in `~/EDF/tmp/flipw/census/src/`.
- Programs are in `~/EDF/tmp/flipw/census/cases/<id>/`.
- Chrome ran through a scratch copy of `scripts/flip/run.mjs` (`~/EDF/tmp/flipw/census/harness/run-multi.mjs`). The copy changes two lines: `repo` is pinned to the main checkout, and every `*.tsx` in the case directory is copied instead of only `App.tsx`, so two-file export cases can run. The rc.21 install and plugin options are the stock ones.
- Raw Chrome output: `~/EDF/tmp/flipw/census/out/chrome1.json`.
- tsc: `node scripts/flip/tsc.mjs ~/EDF/tmp/flipw/census/cases` from the main checkout answered **21 of 21 pass**.
- Every program below gave one Chrome diagnostic in the `render` phase. Nothing was flagged in the `module` phase, and nothing threw.

Line numbers refer to `rust/crates/solid-reactive-ir/src/value_use.rs` at `035286ae0` unless another file is named.

---

## Holes, ranked by severity

### H1 — `export default <literal | creating call>` answers Followed, and nothing walks the importers (export question)

**Confidence: high (argued from code).** Programs `e01-default-literal`, `e05-default-array` and `e02-default-created`.

```tsx
// e05 Src.tsx
import { createSignal } from "solid-js";
const [c] = createSignal(1);
export default [c] as const;          // e01: export default { c };
                                      // e02: export default createMemo(() => 1);
// e05 App.tsx
import a from "./Src";
export function App() {
  try { throw a[0]; } catch (e) { (e as () => number)(); }
  return <p>x</p>;
}
```

- **Chrome:** `STRICT_READ_UNTRACKED` for all three (`render: Reactive value "c" read directly in <mod.App>`).
- **tsc:** passes.

**Why it gets through.**
1. `c` in `[c]` hits `Holder::ArrayElement`, then `position([c], Holder)` (516-530).
2. Nothing in the facts tables holds the `export default` expression, so `holders` is empty and `unheld` runs (440-441).
3. `exported_directly` (1057-1069) checks only that the text from the export's start up to `at.start` trims to `"export default"`. That is true for any expression placed right after `export default`, not just a name. The arm was meant for `export default c`, but it also swallows a holder literal (e01, e05) and a creating call reached through `created_use` (e02: 343-377, then 373, then `unheld`).
4. The answer is `Followed`.
5. Nothing then walks the importing file:
   - Cross-file references are walked only from a `Holder::Binding` (`bound`, 829-851), or through the source symbol's own references (`reactive_analysis.rs:399`).
   - An unnamed default-export literal or call has no binding and no symbol, so the importer's `a`/`o`/`m` is never visited.
   - `a[0]` is not a source. The `catch (e)` call has no row, which is ADR 0692's own example of a form with none.

This is a cousin of item 1c: the default-export arm treats a whole expression as a followed export.

The only remaining doubt, for e01 only: `enclosing_literal` needs `{ c }` in `object_get_shapes`. If it is missing there, e01 answers `value-in-carrier` instead. e05 (an array, `array_literal_at`) and e02 (a call) do not depend on that.

**Smallest fix.** In `unheld`, take the `export default` arm only when `mode == Mode::Value` and `at` is an identifier reference (`is_value_reference(file, at)`). In every other case (a literal holder, a root holder, a creating call), answer `Open`: `value-use-unclassified`, or better a named `value-exported-unnamed`. The alternative is to walk the default export symbol's references in importers. Add e01, e02 and e05 to `value-use-census` (Importer side).

### H2 — Inside a parameter, prop or context hop, a walked call needs no row (`Callee`, 480-485)

**Confidence: medium-high for the parameter form (argued from code).** Program `p01-param-pattern`.

```tsx
import { createSignal } from "solid-js";
function run(f: () => number) {
  const o = { f };
  const { f: g } = o;
  return g();
}
export function App() {
  const [c] = createSignal(1);
  run(c);
  return <p>x</p>;
}
```

- **Chrome:** `STRICT_READ_UNTRACKED` (`"c" read directly in <mod.App>`).
- **tsc:** passes.

**Why it gets through.**
1. The path: `run(c)` → `argument()` (1007-1039) → project callee, identifier parameter, so `parameter_hops = 1` → shorthand `f` → `ObjectValue` → literal `Holder` → `Binding(o)` → the pattern binding, `each = Value` (817-821) → `g` → `Holder::Callee`.
2. The condition at 480-485 demands a row only when `parameter_hops == 0 && prop_hops == 0`, so `g()` answers `Followed` with no `has_row` and no `tracked` check.
3. This is exactly the shape ADR 0692 lists as having no row (`const { c: h } = o; h()`). The fix at 487-488 covers it only at hop depth 0.
4. The comment's premise ("a parameter's call has its row at the one call that hands it") holds only where interproc resolves the callee to the parameter. A pattern over a holder is where it does not.
5. The handed net does not cover it either. Parameters are not handed in `handed_value`. Also, a shorthand holder never counts as handed: `local_access.rs:315` passes `shorthand_binding`, a *declaration* span, and the Reference check at `local_access.rs:326` then answers false.

**Same bypass in the other hops (Chrome confirms all of them).**
- `p06-ctx-pattern`: `const v = useContext(Ctx); const { v: w } = { v }; w();`. `context_readers` sets `prop_hops`. **Medium:** this one depends on whether context transport reads through a destructured literal.
- `p05-prop-pattern`: **held** by the handed net (see "What held").

**Smallest fix.** Exempt only the hop's own position. Record the spans a hop starts from (the parameter's references, the `props.<prop>` member, the `useContext` call) and skip the row check only when `ast.calls[call].callee` is one of them. Any further alias or pattern inside the hop then needs `has_row` or `tracked`, as at depth 0.

### H3 — A tracked position excuses any accessor, async ones too (485, 589-599)

**Confidence: medium-high (argued from code).** Program `p08-async-tracked-pattern`.

```tsx
import { createMemo } from "solid-js";
export function App() {
  const c = createMemo(async () => { await new Promise((r) => setTimeout(r, 20)); return 1; });
  const o = { c };
  const { c: h } = o;
  return <p>{h()}</p>;
}
```

- **Chrome:** `ASYNC_OUTSIDE_LOADING_BOUNDARY`. The direct control `p09-async-tracked-direct` (`{c()}`) gives the same code.
- **tsc:** passes.

**Why it gets through.**
1. `h()` is reached at hop depth 0 with no row (the ADR's pattern over a holder).
2. `self.reading && self.tracked(h)` holds because tracked JSX does not `reports_untracked_read`, so the call answers `Followed`.
3. `tracked` speaks only for the strict-read rules. A pending async read at a tracked position is SC5003's (and SC5001's) business. ADR 0771 says only a *settled* accessor is legal there, and those rules need a read row with the source.
4. The handed net misses it too, for the same shorthand reason (`local_access.rs:315`, then 326).
5. Result: no row, no obligation. The checker should answer at least an uncertifiable (App is exported, so SC5003 would be U rather than V). It answers clean.

**Smallest fix.** Give `Census` the async source set: `async_sources`, as `HandedTables::async_only` already uses. Apply the `tracked` escape only when the walked accessor is not async. Otherwise require `has_row`.

### H4 — A store or props root's member never needs a row past the first step (`member`, 606)

**Confidence: medium (argued from code).** Programs `p11-store-holder-member` and `p15-prop-store-alias`.

```tsx
// p11
import { createStore } from "solid-js";
export function App() {
  const [st] = createStore({ a: 1 });
  const o = { st };
  void o.st.a;
  return <p>x</p>;
}
// p15
function Wrap(props: { st: { a: number } }) { const k = props.st; void k.a; return <p>x</p>; }
export function App() { const [st] = createStore({ a: 1 }); return <Wrap st={st} />; }
```

- **Chrome:** `STRICT_READ_UNTRACKED` for both (`Reactive value "a" read directly in <mod.App>` / `in <Wrap>`).
- **tsc:** passes.

**Why it gets through.**
- p11: `st` → `ObjectValue` → `RootHolder` → `Binding(o)` → `o.st`. `member(RootHolder)` is not carried, so it continues with `position(o.st, Root)` (627-632). Then `MemberObject(o.st.a)` gives `member(Root)`, which returns `Followed` at 606 with no row test.
- p15: the same through `prop_reads(Root)` and an alias of `props.st`.
- `has_row` is never asked in `Mode::Root`. The `Callee` arm also requires `mode == Mode::Value` before it demands a row.
- So the census trusts "its member Gets and Sets have their rows" (36) even when the root arrives through a literal slot or a prop alias, where no store-read row names it.
- Doubt: a store-read row may still exist through a props or transport path I could not trace without running.

**Smallest fix.** In `member()` for `Mode::Root`, past the walk's first position, require a read or write row over `member.span` (`has_row`'s containment test on the member span). Answer `value-held-in-binding` or `value-in-carrier` otherwise. A root alias that is itself a source keeps its own walk, so it stays followed.

### H5 — `Holder::JsxChild` is Followed whatever the parent element (493)

**Confidence: medium (argued from code).** Programs `p14-for-child` and `p13-children-store`.

```tsx
// p14
import { For, createSignal } from "solid-js";
export function App() { const [c] = createSignal(1); return <For each={[1]}>{c}</For>; }
// p13
function Wrap(props: { children: { a: number } }) { const k = props.children; void k.a; return <p>x</p>; }
export function App() { const [st] = createStore({ a: 1 }); return <Wrap>{st}</Wrap>; }
```

- **Chrome:** `STRICT_READ_UNTRACKED` for both (`"c" read directly in <For>`; `"a" read directly in <Wrap>`).
- **tsc:** passes.

**Why it gets through.**
1. An attribute goes through `jsx_value`, which handles the tag, provider and `prop_reads`. A child expression answers `Followed` at 493 without looking at its parent.
2. For a known primitive (p14), `argument()` would have opened the same value at 965-968, because `For`'s mapper is a strict window (`reports_untracked_reads_at`). The JSX form skips that test.
3. For a project component (p13), the child is the `children` prop. It is never walked to `props.children`, so `k.a` is never seen.
4. I found no read producer for a bare accessor handed as a JSX child. `jsx_child_expressions` has no consumer outside `value_use.rs`, and function-child roles key on function literals (`execution_role/props_shape.rs:261`).
5. Doubt: compiler `jsx_operations` may give `{c}` under `<For>` a row I could not see. A plain `{c}` under an intrinsic element is tracked insertion and fine.

**Smallest fix.** Route `JsxChild` through `jsx_value`:
- an intrinsic element stays `Followed`;
- a project component goes through `prop_reads(component, "children", mode)`;
- a known primitive asks the dialect's children slot with the same `callback_may_open_strict_window` / `reports_untracked_reads_at` tests `argument()` uses (or answers `value-to-unresolved-callee`);
- an unresolved tag is `Open`.

---

## Smaller observations (not confirmed holes)

- **`symbol_references` fails open.**
  - `SemanticLookup::symbol_references` (`indexes.rs:2775`) answers an empty list for a symbol Type Facts does not know. `symbol_references_if_present` exists precisely because "a missing symbol is missing reference evidence".
  - The census uses the defaulting form at `reactive_analysis.rs:399`, `bound` 834-836 and `context_readers` 755. `bound` also skips the cross-file walk when `entities().exact(..)` is `None` (830-833).
  - I found no reachable case. It is still a silent path on missing evidence. It should answer `value-use-unclassified` at the source's declaration.
- **`typed_sources` is not in the census loop** (`reactive_analysis.rs:371-376`), although `open-work.md` now says `typed_sources` views "rest on the census". A proven source handed to a typed parameter is walked through `argument()`, so this matters only for a typed source no proven source reaches.
- **The handed net's shorthand.** `local_access.rs:315` feeds `shorthand_binding`, a declaration span, back into `handed_value`, and the Reference check at 326 then makes every shorthand holder unhanded. This is outside the census, but H2 and H3 lean on the net staying closed.
- **`has_row` containment runs both ways** (580-584). A read, write or action row whose location *contains* the callee counts as the callee's row, so a call of an alias nested inside another source's call row inherits that row. I built no Chrome case where the outer row is silent and the inner read is a defect.

---

## What held

| Attack | Program / form | Stopped at |
|---|---|---|
| Default inside `export function App`, `export default function App`, `export const App = () =>`, `export default () => {…}` | `throw c` in each | `exported_directly` 1057-1069: the specifier must equal `at`, or the head must be exactly `export default`. Unit test at 1170. |
| `export default (c)`, `export default c \|\| x` | — | Head is `export default (`, so `unheld` → Open (470). A `LogicalValue` with no row holding the composite → Open (540-556). |
| `export { c }`, `export { c as d }`, `export default c`, importer misuse (`e06`, `e04`, `e03`, Chrome `STRICT_READ_UNTRACKED`, tsc pass) | importer `throw d` | The importer's reference is one of the source symbol's references (`reactive_analysis.rs:399`), assuming Type Facts resolves import aliases, which `handed_value`'s import note relies on too. Its `throw` → `unheld` → Open. **Producer-dependent; not verified without a run.** |
| Pattern over a holder at hop depth 0, untracked | `const { c: h } = o; h()` | `Callee` 480-488 → `value-held-in-binding`. |
| Prop alias / prop pattern | `p04` `const g = props.f; g()`, `p05` `const { f: g } = { f: props.f }` | Census `Followed` (prop hop), but `local_access.rs` `discover_call_as` takes ADR 0511's closed prop alias (`exact_props_callee` / `handed_member_alias`). The non-shorthand `props.f` makes `handed_value` true, which gives `handed-member-dispatch`. |
| Parameter alias / parameter holder | `p02` `const g = f; g()`, `p03` `run({ c }); o.c()` | Census `Followed` (hop). Likely covered by the transport's `parameter_inputs` / `Value::Object`. **Low certainty; probe these when a build exists.** |
| Store pattern over a literal | `p10` `const { st: s } = { st }; s.a` | ADR 0463's exact alias makes `s` a source with its own rows. |
| `props.children()` with an accessor child | `p12` | `exact_props_callee` → `discover_props_call` (`local_access.rs`). |
| `.call` / `.apply` / `.bind` through a cast, `.bind` not invoked on the spot | — | 637-647. |
| `<H />` with `H` holding the accessor; `<C />` with the source itself | — | `unheld` → `jsx_name` → `value-to-unresolved-callee` (458-465). Oxc walks a component tag name as an `IdentifierReference`, so `is_value_reference` holds. |
| Method or slot returns, IIFE, a second hop, a literal in a literal, `arguments`, a root in a helper | — | 902, 882-884, 710 / 894 / 1025, 518, 1015, 607 / 811. |
| Recursion / cycles | — | `walked` with `None` while answering → Open (432-437). `MAX_STEPS` → Open (428). |

**Strongest thing that held:** the 1c fix itself. No position *inside* an exported function or arrow reaches `Followed` through the export arm, because `exported_directly` matches only an exact specifier or a bare `export default` head. The default-export hole (H1) is the same arm misfiring on a whole expression, not a position inside an exported declaration.

## Not done

- No checker run (forbidden): every "certifies" is argued from code.
- I did not trace interproc summaries far enough to rule out a read row at `run(c)` (H2) or a transport row through a destructured literal (`p06`). These are the first things to confirm with a post-`e40429f5e` build: `p01`, `p06`, `p08`, `p11`, `p13`, `p14`, `p15`, `e01`, `e02`, `e05`.
- No fuzz shapes were written.
