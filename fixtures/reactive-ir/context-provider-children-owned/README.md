# Context provider children, the owner codes (ADR 1147, arm 1)

rc.21's provider is `createRoot(() => { setContext(provider, props.value); return children(() => props.children); })`. A child of a provider runs inside
the `children()` memo's computation, whose owner is that memo (created under the provider's root), wherever and whenever the memo is read. So the owner
codes at a provider's child (SC3001's PRIMITIVE_IN_FORBIDDEN_SCOPE and CLEANUP_IN_FORBIDDEN_SCOPE, SC6002's PRIMITIVE_IN_EFFECT_CALLBACK) do not depend on
where the provider's value goes, and arm 1 (`context-provider-children-owned`) answers them: the leaf universe's hand-off question places a child of an
exact provider (`callback_return::exact_context_provider`: the tag is the file's own unwritten, unexported `const` initialized by the dialect's
`createContext`, no spread, no `children` attribute, no direct `eval`). The stack codes (a flush: SC3001's flush code, SC2006, SC2008; SC1002) are not
answered: the stack is whoever reads the memo, and the flush and after-await accounting never ask in this mode.

- Positive (the child's creation and cleanup rows end): `ReturnedApp` (the provider is the component's return), `StoredApp` (the provider element stored and
  handed to an unread component: still the memo's owner), `ThroughApp` (a project component places `props.children` in the provider: textbook
  `id-context`'s shape).
- Negative, F2 (the tag is no exact provider, rows stay): `WrittenApp` (a written `let`), `PatternApp` (a pattern's name: the unread component),
  `ImportedApp` (another module's context).
- Negative, the stack codes (rows stay): `FlushStoredApp` (the review's k11: a flush in the child, the provider's value stored and handed to the unread
  component), `FlushInnerApp` (k9: the same one component down).
- Negative, the graph still decides: `LeafApp` (the provider created in a tracked effect: the child's entry has a leaf bit) and `HandlerApp` (created in a
  handler: an unowned entry, SC6002 declines).

Stub: `context-provider-children-refusals`'s, plus `onCleanup`, `createTrackedEffect` and `flush` from the published rc.21 typings. Chrome: the runtime-oracle
case `context-provider-children-owned` (the owner codes quiet where a package reads the provider's value in a tracked effect or an effect's apply).
