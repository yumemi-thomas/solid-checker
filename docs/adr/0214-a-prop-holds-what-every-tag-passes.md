# ADR 0214: A prop holds what every tag passes

- Status: accepted and implemented (2026-10-06). Phase 4 of the value-flow
  work on caller-supplied member dispatch.
- Owners: `CallGraph::prop_holds_builtin`, `CallGraph::rendered_only_through_jsx`
  and `CallGraph::value_holds_builtin` (`solid-reactive-ir/src/attribution.rs`).
- Relation: extends ADR 0213 from call arguments to JSX attributes.

## Context

A component often hands a prop to a helper:

```tsx
const BarChart = (props) => {
  const empty = () => isPlotEmpty(props.series);
  …
};
<BarChart series={seriesOf(chart())} />
```

The obligation at `isPlotEmpty(props.series)` asks which `some` runs on the
prop. The prop's value is whatever the rendering tags pass.

## Decision

In a closed program (ADR 0193), a parameter-member obligation whose argument
is `props.name` is discharged when every one of these holds:

1. **`props` is the component's own.** It is the only parameter of a function
   around the argument, a plain identifier with no default. Nothing in its
   file assigns it or a member of it. `children` is the element's content and
   is not followed.
2. **The component is rendered only through tags.** Every enumerated entry is
   a JSX element the graph resolves to it. Every reference to the component
   is one of those tags, its declaration, or its module surface. A call, a
   dialect renderer (`createComponent`), a rendering prop
   (`component={Panel}`) or any other value escape supplies props no tag
   shows, so the proof fails.
3. **Every tag passes a built-in value.** A tag that spreads attributes may
   set the prop, so it fails the proof. A matching attribute is a string, a
   boolean, or an expression that holds a built-in value: by its origin
   (ADR 0211, 0212), or as a parameter (ADR 0213) or a parent's prop that
   holds one on every entry. A missing attribute is `undefined`.

The member must not be assigned anywhere, nor `__proto__` (ADR 0211).

## Consequences

- Props that only ever receive values built in the program select the
  built-in members at the component's helper calls. Chains of forwarded
  props are followed, four levels deep with parameters.
- A component that is also rendered by a router, a list primitive's
  rendering prop or `Dynamic` keeps its obligations.
- Still open:
  - destructured or merged props (`splitProps`, `mergeProps`);
  - tags with spreads;
  - props passed through a package component;
  - open programs.

## Evidence

- **Fixture** `fixtures/reactive-ir/prop-pass-through-dispatch` (a closed
  program):
  - discharged: `List`, rendered with `items={[1, 2]}` and by `Middle` with
    `items={props.items}`, whose only tag passes `[3]`;
  - kept: a component rendered with a spread, one with a tag passing the
    unrendered root's prop, and one exported in an array.
- **Coverage:** 183 fixture projects, 982 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-u-browser.json` (ADR 0213):
  - violations unchanged at 285;
  - uncertifiable 3,318 to 3,299: 19 sites removed, none added.
  - For example, kui's `BarChart` receives `seriesOf(chart())`, an array
    literal, and `[...SERIES]` in its test.
