# owner-callback-prop

**Claim (ADR 0562).** An arrow written as a prop of a same-file project component that only ever calls the prop (ADR 0538's proof: the props parameter is one identifier read as static members, or an object pattern binding the prop to one identifier) runs at each call the component writes in its own body or JSX, under the component's owner. The owner pass gives the arrow that edge, so SC4002's provider proof can run.

Violations (`missing-context-provider`): `BodyCall` (`props.run()` in the body), `DestructuredCall` (`{ run }`, `run()`), `JsxCall` (in a render expression, read through the namespace).

Silent: `ProvidedParent` (a provider above the child) and `OwnedCleanup` (`onCleanup` in a prop the child calls in its body is owned). `Kid` is rendered by three parents with three arrows; each arrow is answered on its own.

Obligations (`missing-owner` uncertifiable; SC9012 where the dispatch net fires): `NeverCalled` (a prop only tested runs nothing), `Forwarded`, `Copied` (a copy called from a timer), `SettledCall` (a call inside the child's `onSettled` callback, under that callback's owner), `OtherFile` (a child in `remote.ts`).

Premises: the closed program is the `private` manifest. Runtime: rc.17 throws ContextNotFoundError at each SC4002 line (fuzzer cells `mx-recall` CONTEXT_NOT_FOUND `cbp-body`, `cbp-body-destructured`, `cbp-jsx`, `cbp-jsx-destructured`).
