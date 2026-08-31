# A value constant re-exported beside a callback-forwarding function stays closed

`index.js` is a barrel — `export * from "./inner.js"` — that republishes two
siblings declared in the same module:

- `subscribe(client, cb)` forwards its caller-supplied callback into an
  unresolved member dispatch (`client.getThing(cb)`). The callee resolves to no
  declaration, so the callback's execution is uncertifiable and the analyzer
  records a contract-generation obligation (`UnknownCallbackExecution`) whose
  subject is `subscribe`. The proposal's unresolved-claim census carries a
  `callbacks` domain entry for `subscribe`, and its summary stays `callable`.
- `PREFIX` is a plain string constant. It must certify as a `value` with no
  function effects: no `callbacks` claim, summary `shape: "plain"`.

The trap this pins: the obligation's owning function and the value constant are
published through the *same* wildcard re-export, so a naive attribution that
reaches from the obligation to every export sharing the barrel's runtime
identity would open `callbacks` on `PREFIX` too. A closed value export with
function effects is then refused outright by the operation-graph invariant
(`package contract value export .:PREFIX cannot have function effects`).

Here `subscribe` is a named export, so its obligation carries an exact compiler
symbol that resolves only to `subscribe`; a value constant never carries a
function's symbol, so `PREFIX` is never marked. This fixture is the
integration-level companion to the unit regression
`contract_generation_callback_attribution_tests::exact_function_symbol_excludes_value_siblings_that_share_runtime_identity`
in `rust/crates/solid-facts-backend/src/main.rs`.

The related ecosystem-wrapper failures — `@tanstack/solid-db` (`IR`),
`@tanstack/solid-hotkeys` (`ALL_KEYS`), `@tanstack/solid-query`
(`dataTagErrorSymbol`), `@tanstack/solid-form` (`initialServerFormState`), and
`@tanstack/solid-query-persist-client` (`PERSISTER_KEY_PREFIX`) — take the
*symbol-less* path: the callback forwarder is an anonymous function inside the
wrapper, so its obligation has no symbol and falls back to the runtime identity
shared across a `export * from "<dependency>"` barrel. That case is pinned by
the sibling unit regression `an_empty_function_symbol_never_falls_back_onto_a_value_sibling`;
it cannot be a self-contained corpus fixture because the shared runtime identity
only arises from a real cross-package `export *` composition.
