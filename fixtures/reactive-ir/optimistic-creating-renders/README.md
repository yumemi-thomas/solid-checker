# optimistic-creating-renders

**Claim (ADR 1216 slice 2).** SC2009's existence walk (`some_entry_on_stack_of`, slice 1) also climbs a callback prop's function
to the child's straight-line calls of the prop (`prop_literal_call_sites`, ADR 1171 and 1190, a child that nothing else names: inline, named, destructured,
forwarded, spread), and a component to the elements that render it eagerly (`eager_render_sites`): its tag written as a direct
child of intrinsic elements only, not in an attribute or an expression child, and not in a component's children.

| component | SC2009 |
|---|---|
| `CallbackProp`, `RenderedChild`, `ForwardedProp` | violation (main: uncertifiable) |
| `ConditionalRender` (`{on() ? <Late /> : null}` renders after the click), `PropInHandler`, `InComponentChildren` | uncertifiable |

Chrome throws at all three violations and in `InComponentChildren` too (`Box` reads `props.children`, which renders it): that
uncertifiable is a recall miss, not a claim. Chrome: `fixtures/runtime-oracle/optimistic-creating-renders`. Stub: `optimistic-creating-entries`' with `createSignal`.
