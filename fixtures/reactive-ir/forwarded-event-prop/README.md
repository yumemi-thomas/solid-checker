# forwarded-event-prop

**Claim (ADR 0199).** A read in a function literal written as a project
component's prop runs on DOM event dispatch, never in the caller's strict-read
window, when the component only ever hands that prop to an intrinsic element's
event handler. The read then takes the role of a handler written on the element
itself (`ExecutionRole::EventCallback`) and is not reported
(`execution_role::forwarded_event_prop_role`).

The component (`buttons.tsx`, imported, plus one local) is resolved exactly from
the tag. Its props parameter must be a single identifier. That parameter, and
every `const` bound to a `merge` or `omit` view of it, may be used only as a
static member access, a spread, or the props argument of such a view. Every
`props.onPress` must be:

- the value of an intrinsic element's event attribute (`Direct`);
- inside the function written as such a handler (`Wrapped`, and `Merged`
  through a `merge` view); or
- the value of another project component's prop that satisfies the same proof
  (`Chained`); or
- inside a literal written as such a prop (`Confirm`, which hands
  `() => props.onConfirm?.()` to `Direct`).

A `const` alias of the props or of a view, through a type assertion, is the
same object under another name (`AliasSpread`).

Code nested anywhere in a literal that runs only on dispatch runs only after
it: a callback that literal hands to an async helper is clean too
(`NestedInHandler`), where the same call in the body is not
(`NestedOutsideHandler`).

A spread onto an intrinsic element carries an `on…` prop to an event
listener (`Spreads`, and `OmitSpread` through an `omit` view): rc.13's
`@solidjs/web` `assignProp` attaches an un-namespaced `on…` property with
`addEvent`.

| Case | Finding | Why |
| --- | --- | --- |
| `UsesDirect`, `UsesWrapped`, `UsesChained`, `UsesLocal`, `UsesSpreads`, `UsesMerged`, `UsesOmitSpread`, `UsesConfirm`, `UsesAliasSpread`, `NestedInHandler` | none | the literal reaches only an event listener |
| `UsesCallsDuringRender` | `SC1001` uncertifiable | the component calls the prop while rendering (a true defect, but this proof does not make claims) |
| `UsesSpreadsToCaller` | `SC1001` uncertifiable | spread onto that same component |
| `UsesConfirmEagerly` | `SC1001` uncertifiable | `Confirm`'s literal handed to that same component |
| `NestedOutsideHandler` | `SC1001` uncertifiable | an async helper's callback written in the body |
| `UsesMergeEscapes` | `SC1001` uncertifiable | a `merge` view is handed to a helper |
| `UsesSpreadsRender` | `SC1001` uncertifiable | a spread prop whose name is not an `on…` name |
| `UsesKeeps` | `SC1001` uncertifiable | the prop is invoked from a timer |
| `UsesMixed` | `SC1001` uncertifiable | one use is an event, another runs while rendering |
| `UsesLoop` | `SC1001` uncertifiable | a forwarding cycle proves nothing |

The findings in `buttons.tsx` itself (`reactive-handler-frozen` on `Direct` and
`Mixed`, and the read in `CallsDuringRender`) belong to other rules and do not
move.

`solid-js.d.ts` is copied from `fp-exec-handler-writes`, with three changes:

- `merge` and `omit`, and their types, are copied from `@solidjs/signals`
  rc.13's `store/utils.d.ts` (the proof resolves them as the dialect's props
  merge and split);
- its `button` element accepts a handler with an event parameter;
- `button` and `div` accept any attribute, so that the spread cases
  type-check.

No proof here depends on the stub's element typings.
