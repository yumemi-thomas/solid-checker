# context-package-entries

**Claim (ADR 0949).** A project reference that hands a package symbol on, where no call and no tag names the entry, is an SC4002 **obligation at the reference** when the package holds a read of a context it created with no default that nothing proves provided. The reference may run the function, method or getter the symbol holds, under an owner, and Solid throws `ContextNotFoundError` where no provider is above it. Before ADR 0949 each of these was certified clean (Chrome: CONTEXT_NOT_FOUND).

`App.tsx`, each exported component hands one on:

- Obligation, at the reference: `Handed` (`createEffect(useUi, …)`), `Dynamically` (`<Dynamic component={Panel}>`), `Prop` (`<Layout comp={Panel} />`, a project component that renders `<props.comp>`), `Child` (`{Badge}`, a function child), `Method` (`new Reader().read()`: a class method is entered by member dispatch no reference to its name names, so the holder `Reader` is the reference), `Getter` (`registry.current`: the holder `registry`), `Member` (`<Ui.Panel>`, an object's member as a tag: the holder `Ui`).
- Silent: `PureHanded`, `PureChild`, `PureDynamic`: the same forms for `dep-ui-pure`, a package that reads no context at all (nothing a reference could run reads one).

A reference is not an entry when it is the callee of a call or the tag of an element that resolves exactly to a function (the call walk and the render walk decide those: `context-package-component`, `context-package-no-default`), an import or export specifier, a type position or query, a context binding (the project's own read is its own row), or written in a function proven never to run. Which function the value runs is not followed, so every other reference to a symbol a seeded package declares is the obligation: precision is traded for the zero false clean.

Package: `node_modules/dep-ui-context` is this fixture's own (see `context-package-component`: `Panel`, `Badge`, `useUi`, class `Reader`, `registry`'s getter and the `Ui` object all read the default-less `UiContext`); `dep-ui-pure` has no `solid-js` import. `solid-js.d.ts` abbreviates `Dynamic`.

Chrome (rc.21 development build, the runtime oracle): the seven positives each throw ContextNotFoundError; the three silent forms throw nothing.
