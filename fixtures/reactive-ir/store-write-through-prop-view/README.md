# store-write-through-prop-view

**Claim (ADR 1211).** ADR 1172 proves a function-local store handed by prop still holds its own literal when the component never
reads the handed setter and reads the store only as the root of a contained use. Its census of the prop's reads now follows the props
object through a `merge` view or an `omit` view held in a `const` (an `omit` rest spread onto an element reads no key it drops), and
through a whole-props spread onto a component of the file (that component's reads). A store and setter handed in the closed value
literal of a provider of a context the file declares and never exports are read through every `useContext` result (by static keys, or
destructured). A write past the store's root is then dropped outside the setter: STORE_WRITE_OUTSIDE_SETTER.

| component | outcome |
|---|---|
| `MergeChild` (`merge({ extra: 1 }, props).s1.a.v = 7`), `OmitChild` (`omit(props, "extra")`), `ForwardChild` (via `<ForwardChild {...props} />`), `ViewSpreadChild` (`merged.s1.items.push(1)` beside `<p {...omit(props, "s1", "setS1")}>`) | SC2003 violation (main: uncertifiable `store-write-unproven`) |
| `ContextChild` (`useContext(ValCtx).s1.a.v = 7`), `ContextDestructuredChild` (`const { s1 } = useContext(PairCtx)`, `items.push`) | SC2003 violation (main: uncertifiable) |
| `MergeCallsSetterChild` (the setter called through the view), `MergeHandedChild` (the view handed on), `ForwardCallsChild` (the setter called after the forward), `ContextCallsSetterChild`, `ContextHandedChild` (the context value handed on), `ContextExportedChild` (an exported context) | uncertifiable, as before |

Chrome: `fixtures/runtime-oracle/store-write-through-prop-view`. Stub: `store-write-through-prop`'s, with `merge` and `omit` as rc.21's
`dist/types/store/utils.d.ts` types them.
