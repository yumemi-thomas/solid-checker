# ADR 0326: A component's function child is its `children` prop

- Status: accepted and implemented (2026-10-19).
- Owners: `component_child_function`, the `within` scope of
  `execution_role_where` and the tracked-region filter in
  `semantic_role_before_awaits`, `child_literal_invoked_during_render` in
  `callee_callback_timing`, and the JSX-child arm of `InvocationFlow::uses`
  (`solid-reactive-ir/src/execution_role.rs`).
- Relation: places a component's function child the way ADR 0204 and ADR 0319
  place a function attribute. Control-flow children callbacks
  (`control_flow_execution_role`) are unchanged.

## Context

The runtime oracle (2026-10-19, `fixtures/runtime-oracle/oracle-children-body`)
found an unproven clean claim. `Kids` calls its function child in its body:

```tsx
function Kids(props: { children: (x: number) => JSX.Element }) {
  const el = props.children(1);
  return <div>{el}</div>;
}
<Kids>{(x) => { console.log(count()); return <span>{x}</span>; }}</Kids>
```

Solid warns `STRICT_READ_UNTRACKED` at `count()`, and the checker said
nothing. The same callback as a `render` prop was a proven `SC1001`.

The compiler fact projects a component's `EagerOnce` child as a `Deferred`
callback role (`solid_facts::compiler`, "a function value's body is invoked by
the receiving component"). The IR read that role as `DeferredCallback`, a
role that reports nothing. It describes who invokes the function, not when.
An unresolved component's child (en-passant has no `node_modules`, so its
`<For>` is a plain tag) and a library component's child were silent the same
way.

## Decision

A function literal written as the whole child expression of a non-intrinsic
element (`component_child_function`) is the component's `children` prop.

1. Its compiler `Deferred` role places nothing. A tracked region around the
   element (`{shown() && <Kids>{…}</Kids>}`) does not place it either: that
   region lowers the element, it does not run the child. Inside such a
   function, only compiler facts strictly inside it place code.
2. It runs during the receiving component's render when that component's own
   body calls `props.children` directly: ADR 0204's
   `component_invokes_prop_in_body`, with `children` as the prop. The element
   must have exactly one child, no spread and no `children` attribute. This is
   a positive proof only. When it does not hold, the existing timing logic
   answers, and a control-flow component's callback keeps its proof.
3. ADR 0319's walk follows a component's sole child into the `children` prop,
   under the same conditions, so a child the component calls in tracked JSX
   or an event handler takes that role.

Anything else keeps the lexical rendering role with unproven timing:
`SC1001` uncertifiable ("passed to a consumer that is not proven to invoke
it").

## Proof obligations

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| the child is a function literal | a `FunctionFact` with exactly the child expression's span | an expression child (`{count()}`) is the compiler's getter, unchanged | `ExpressionChild` |
| the element is a component | not `intrinsic_element` | an intrinsic element's function child is inserted by the compiler | `IntrinsicChild` (unchanged) |
| the compiler's `Deferred` places nothing | callback filtered in `source_execution_role` | the projection's own comment: the receiver invokes it | `BodyChild`, `OpaqueChild` |
| an enclosing region places nothing | regions containing the child function are skipped | the region's tracking pass evaluates the element, not the prop's body | `BodyChildInRegion` |
| body invocation | sole child, no spread, no `children` attribute, `props.children(…)` called directly in the resolved component's body | ADR 0204 | `BodyChild`; `SpreadChild` refuses |
| flow through `children` | sole child, no `children` attribute, component resolved and current | ADR 0319 | `TrackedChild`, `EventChild` |
| `props` does not escape | every reference to the props parameter in the component is a static member read that is not written, deleted or called, except the call of the prop itself; no member named the prop (or a computed one) is written or deleted in the body; applies to ADR 0204's attribute proof too | an alias, spread, argument (a single-source `merge(props)` is `props`), destructuring, `delete`, or a sibling prop method given `props` as `this` could replace the prop before the call | `AliasedChild`; review rounds 1-2 (`merge`+`Reflect.set`, `this` method, `delete`); costs `dynamic-string-tag-events` `UsesCallingRoot` (now uncertifiable); corpus: no movement |
| every write form counts | the parameter is not written or redeclared (`binding_written`); member writes are read from assignments, `write_targets` (destructuring leaves, updates), `iteration_targets` and `deleted_targets` | a loop head or `var` redeclaration writes as an assignment does | `LoopChild`, `RedeclaredChild` (review round 4) |
| no other name for `props` | no `arguments` or `eval` in the component (parameters included) | `arguments[0]` is `props` in strict mode too | `ArgumentsChild` (review round 3) |
| the call runs in the body | not inside a class written in the component (`class_within`) | a field initializer runs at construction | `FieldChild` (review round 5) |
| the call is reached | a direct call anywhere in the body's own flow, conditional or not | the feasible-path premise every direct body read already rests on (`if (false) n()` is a violation today) | review round 1 `dead-child`, kept by that premise |
| control flow unchanged | the timing branch is positive-only; `control_flow_execution_role` answers first | ADR 0323 | corpus: 0 removed |

## Consequences

Corpus A/B (49 projects, rc.13, against `635ed74d4`, together with ADR 0327):
0 removed, +17. Eleven read uncertifiables are function children whose
invocation nothing proves: library components (Kobalte `Select.Value`, context
menus, tab panels) and en-passant's unresolved `<For>`. Before, they were
silent. Fixtures: `component-function-child` (new); `shadowed-control-flow-tag`
and `prop-head-get` each gain one uncertifiable for their local `For`, which
was silent for the same reason.

The runtime oracle now covers both children reads with proven violations.
